# ADR-006: daimon Owns Its Event Loop

**Status**: Accepted. Supersedes ADR-005 §2 (the channel service thread); ADR-005's other decisions
(one socketpair per agent on fd 3, identity by construction, the bounds) stand.
**Date**: 2026-09-22 (2.3.4)
**Context**: through 2.3.3 daimon served HTTP from inside sandhi's serve loops
(`sandhi_server_run_opts` / `sandhi_server_run_async`). Those loops own the accept loop and give the
caller no turn of its own: no tick, no way to wait on another descriptor. Everything daimon needed
to do between requests had nowhere to run:
- **the agent channels** went on a background thread (2.3.3). From the moment it started, every
  allocation in daimon took the stdlib's heap lock (`lib/alloc.cyr` skips it only while no thread
  has ever started): `alloc(64)` 10 → 53 ns, `json_parse` 429 ns → 1.24 µs,
  `mcp_manifest_100_tools` 141 → 258 µs;
- **a stop held the server** for its agent's grace period (VULN-014; a request sent during a 5 s
  stop waited 4.8 s);
- **delivery waited for HTTP traffic**: the thread handed messages to the main thread, which only
  ran when a request arrived;
- **an agent that exited stayed RUNNING** until someone asked about it.

## Decision

daimon runs one thread and one loop (`server_loop`, `src/server.cyr`) over one **epoll** set:
- the listening socket (non-blocking, close-on-exec);
- every connection still being read, with `recv(MSG_DONTWAIT)`;
- every agent channel, which `src/ipc.cyr` registers and deregisters itself.

A tick runs every `SERVE_TICK_MS` (100 ms; 10 ms while a deferred answer waits). It checks
connection deadlines and frame timeouts, answers deferred requests, and does agent upkeep
(`agents_tick`: reap exited agents, advance stops).

**HTTP stays sandhi's.** A request is complete by the rule `sandhi_server_recv_request` uses
(`sandhi_server_body_offset` + `sandhi_server_content_length`). It is refused by the same three
smuggling checks, and answered through sandhi's senders. Eight malformed and well-formed requests
answered identically on 2.3.3 (sandhi's loop) and 2.3.4 (this one). What changes:
- **A slow client holds only its own slot.** Reads never block. A connection is closed after
  `SERVE_IDLE_MS` (5 s) with nothing sent, or `SERVE_REQUEST_MAX_MS` (30 s) without a whole
  request. Under the old loop one slow client blocked everyone for up to 5 s per read.
- **Responses stay blocking writes**, because sandhi writes a response in one `sock_send`, and
  `SO_SNDTIMEO` bounds them. sandhi set no send timeout.
- **A handler can answer later** (`server_defer`). The stop route uses it: the stop is begun, the
  loop advances it, and the client is answered when the agent is gone (VULN-014).
- **At most `SERVE_MAX_CONNS` (128) connections** are open at once. At the cap the listener leaves
  the epoll set, and new connections wait in the kernel's backlog.
- **Accept errors** follow sandhi's policy (retry, back off 1→250 ms, or stop on a dead listener),
  except that resource pressure never ends the loop. Channels and open connections are still served
  while the listener backs off.

**epoll, not poll.** A poll-based version measured 2.6 µs per pass at 10 open channels, 9.3 µs at
100 and 92.7 µs at 1000, paid by every request. With epoll a pass costs what is ready, not what is
open. The tick's scans are over in-memory records.

**AGNOS keeps sandhi's loop.** It has no agent processes or channels until 2.4.x, and its sockets are
driven differently (`lib/net.cyr`).

## Rejected

- **Keeping the thread** (ADR-005). It carries the allocation lock on everything daimon does, the
  hand-off, and a thread-safety analysis every change must respect. And it still could not fix
  VULN-014.
- **A separate channel process** (the other way out ADR-005 listed). It keeps daimon
  single-threaded, but adds `SCM_RIGHTS` hand-over and a second process to supervise. It fixes
  neither VULN-014 nor delivery without traffic, because the main thread would still have no loop.
- **sandhi's async loop with daimon's tasks on it.** Its per-connection handler blocks in a
  `SO_RCVTIMEO` recv (sandhi documents why: `lib/async.cyr`'s await has no timeout), and it exposes
  no way to add daimon's own descriptors.

## Consequences

- daimon is single-threaded again, and the allocation lock stays unarmed. `tests/ipc.tcyr` checks
  that `_threads_active` stays 0 with channels open. The benchmarks that allocate, run with 100
  channels open, are back to their un-threaded numbers.
- An agent's frame is answered and on the bus as soon as the loop reaches it, whether or not any
  HTTP request arrives.
- A handler that waits on another server runs in a child process (`server_detach`): an MCP call
  forwarded to a registered endpoint, and `web_fetch` / `web_search`. The child keeps only the pipe
  it writes its answer into, which the loop relays; one that has not finished within 60 s is killed
  and its client answered 504. It costs a fork per call (+146.5 µs on a call whose endpoint answers
  at once). The child's memory is a copy, so what the call records (the audit entry) is done before
  it starts. An agent start still runs on the loop: its fork and exec report take milliseconds.
- The loop is daimon's code, not sandhi's. A sandhi release that changes its accept or refusal
  policy has to be mirrored here. The parts it relies on (`sandhi_server_recv_request`'s completeness
  rule, the smuggling checks, the senders) are sandhi's public API.

## Addendum — 2.4.2: when every slot is taken

The loop keeps up to 128 connections (5 on agnos), and a request may take up to 30 s to arrive,
however it trickles. So one local client holding 128 slow connections held every slot, and every
loopback client shares 127.0.0.1, so no per-IP cap could tell it from its neighbours. Measured on
2.4.1: with 128 connections each sending a byte every 2 s, a health check from a new client waited
28.6 s, for the held connections' 30 s deadline.

**Decision**: when every slot is taken and a connection is waiting, the oldest request still being
read gives way to it, once it is `SERVE_EVICT_MIN_MS` (1 s) old.
- A request that arrives at once, as a local client's does, is answered long before then. Only one
  still trickling in can be the oldest.
- The one that gives way is closed without an answer. Writing to a client that is not reading could
  hold the loop for `SO_SNDTIMEO`. Each one is counted (`http_evicted` in `/v1/metrics`).
- Answers being deferred (a stop) or relayed (a detached call) never give way.
- The listener stays armed while a slot could give way.

Measured: with the same 128 connections, the health check is answered in 1 ms, and one held
connection gave way.

*Rejected*:
- **A per-IP cap**: every local client is 127.0.0.1.
- **A shorter request deadline**: the attacker reconnects, and a slow legitimate upload is cut
  whether or not anyone is waiting.
- **A minimum data rate per connection**: it bounds how long each slot is held, not whether all of
  them are.

Eviction acts only under pressure, and only on the request least likely to be anyone's real one.
