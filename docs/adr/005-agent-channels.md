# ADR-005: Agent Channels — an Inherited Socketpair per Agent, Read by a Service Thread

**Status**: Accepted. §2 (the service thread) is **superseded by
[ADR-006](006-own-event-loop.md)** at 2.3.4: the channels are read by daimon's own event loop, and
the thread, its hand-off and its allocation-lock cost are gone. §1, §3 and §4 stand; the byte
figures in §3 are 2.3.3's (2.3.4 frees messages: see the 2.3.4 audit addendum).
**Date**: 2026-09-22 (2.3.3)
**Context**: agents need a way to send messages to daimon. The port carried a socket-FILE endpoint
(`agent_ipc_new` / `bind` / `accept_one` / `send`): one listening Unix socket per agent under a
socket directory, with an `SO_PEERCRED` check on accept. Nothing called it. When it first ran
(2.2.3), it had three defects, left for this step:
- the peer check **failed open** when `getsockopt` failed;
- a message cut short by its sender was queued and ACKed as whole;
- a silent peer held the accept loop (no read timeout).

daimon serves HTTP one request at a time on sandhi's accept loop, which has no hook for other
descriptors.

## Decisions

### 1. Each agent gets a channel at spawn, not a socket to find

`agent_start` makes a `socketpair(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC)`. The child moves its end to
**fd 3** (`AGENT_IPC_FD`), which dup2 leaves without close-on-exec. It then closes every other
inherited descriptor, and execs with `AGNOS_IPC_FD=3` in its environment. daimon keeps the other end.

**Identity is by construction**: only that agent's process received the other end, so a frame on it
is from that agent. The source daimon records is the channel's agent id, whatever the frame claims.

*Rejected*:
- **fixing the socket-file endpoint.** Every agent runs as daimon's own uid, so `SO_PEERCRED`'s uid
  cannot tell one agent from another, or from any other process of that user. Its pid names a
  process that can exit and be reused. The endpoint also brings socket paths, stale files and the
  unlink-before-bind race.
- **agents calling the HTTP API.** It is unauthenticated until 2.5.x, so a message could not be
  attributed to an agent.

The model is AGNOS's own: `chan_op` `CH_MINT` makes an unnamed pair, and `CH_ENDOW` places one end in
the next spawned child. So 2.4.x maps a primitive of the same shape rather than a different design.

### 2. A service thread reads the channels; the main thread owns everything else

A background thread polls every channel, reassembles frames, validates them, and replies at once. An
agent's send is answered while the HTTP server is busy. The thread touches:
- its own channel list and body buffers;
- its own parse **arena**, reset for every frame;
- two bounded thread channels (`lib/thread.cyr`): commands in, records out;
- `alloc()`, for the few records it hands over.

The bus, the agent registry, the audit chain and the logger stay single-threaded: the main thread
takes the records (`ipc_drain`) at the top of every HTTP request.

The frame parse uses bayan's reentrant entry point with an explicit allocator
(`bayan_json_v_parse_ctx_a`), which keeps its state in the call.

**Fork safety**: the main thread forks agents while the service thread may hold the allocator's
lock. The child runs only syscalls and stack buffers from fork to exec (`_agent_child`), so no
lock copied in a held state is ever taken there.

*Rejected*: servicing channels only from the main thread. It runs only when an HTTP request
arrives, so an agent's reply would wait for someone else's request.

### 3. Bounded at every stage

| stage | bound | when it is reached |
|---|---|---|
| frame | 65536 bytes | NACK_INVALID, channel closed |
| a begun frame | 5 s to finish | NACK_INVALID, channel closed (the D-Bus CVE-2014-3639 class) |
| one channel's turn | 16 frames per service pass | the rest wait for the next pass, so one busy agent cannot starve the others (the CVE-2014-3638 class) |
| hand-off to the main thread | 1024 records | NACK_QUEUE_FULL, **checked before anything is allocated** |
| bus queue per agent | 100 messages | dropped and counted |

The shared heap never frees, so a frame's cost to it is measured: 224 bytes for a frame that is
handed over, 0 for one that is refused. Before the room check came first, a refused frame cost the
same 224 bytes, so an agent flooding a full hand-off grew daimon without bound (the journald
CVE-2018-16865 class: allocation driven by socket input).

### 4. A stop or reap closes the channel after reading it

daimon closes its end when it reaps the agent, first delivering what the agent wrote. A child that
inherited fd 3 and outlived the agent can then no longer speak as it. Until then the channel lives
exactly as long as the process: it is added on a successful start and removed at reap.

## Consequences

- **daimon is multi-threaded once an agent has started**. That has two effects.
  - Its epilogue calls `exit_group`: `exit` ends only the calling thread (lib/syscalls says so), and
    daimon would have stayed up with its main thread gone. The two suites that start agents end the
    same way.
  - ⚠ **Every allocation takes the stdlib's shared-heap lock**. `lib/alloc.cyr` skips it only while
    no thread has ever started, and the flag never resets. Measured on the daimon benchmarks, three
    runs each, medians, thread up versus not:
    - `alloc(64)`: 10 → 53 ns;
    - `json_parse`: 429 ns → 1.24 µs;
    - `http_body_read3`: 1.29 → 2.75 µs;
    - `mcp_manifest_100_tools`: 141 → 258 µs;
    - paths that do not allocate are unchanged.

    The thread starts at the first agent start, so a daimon that never starts one pays nothing.
    Ways to remove the cost are on the roadmap (2.3.x follow-ups): a separate channel process
    instead of a thread, or a hook in the serve loop to run the service on the main thread.
- **Delivery waits for HTTP traffic.** A reply (ACK) is immediate, but routing onto the bus happens
  at the next request. With no requests, the hand-off fills and agents are told NACK_QUEUE_FULL.
- **Every compilation unit that includes `src/agent.cyr` must include `src/ipc.cyr`** (and
  `lib/thread.cyr`). cyrius makes an undefined call only a warning when nothing reaches it, and an
  error when something does.
- The wire protocol is in [docs/guides/agent-ipc.md](../guides/agent-ipc.md). Its evidence and threat
  analysis are in the 2.3.3 addendum to
  [docs/audit/2026-09-22-agent-lifecycle-audit.md](../audit/2026-09-22-agent-lifecycle-audit.md).
