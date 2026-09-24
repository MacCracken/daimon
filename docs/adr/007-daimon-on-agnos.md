# ADR-007: daimon on AGNOS — the kernel's own primitives, a polled loop, and gaps filed rather than designed around

**Status**: Accepted
**Date**: 2026-09-23 (2.4.0)
**Context**: AGNOS is daimon's primary target, but through 2.3.4 daimon only *built* for it. There an
agent start answered 501, and the HTTP API ran on sandhi's sync loop. Mapping daimon's agent lifecycle
onto the agnos kernel (1.57.5), and testing it on that kernel for the first time, turned up two kinds
of problem:
- Primitives that exist but have their own shape. `spawn_path`#43 is one line split on spaces. A
  `chan_op`#97 channel carries 64-byte records. `accept` and `recv` never block. A child inherits its
  parent's whole fd table.
- Gaps: nothing ends another process; there are no resource limits; `sleep_ms`#41 holds the CPU;
  inbound SYNs are dropped in interrupt context; connection ids have no owner; and so on.

## Decisions

### 1. Use the kernel's primitives directly, behind daimon's existing seams

The seams are the platform arms that already existed for this: `agent_spawn_with_limits` /
`agent_start`, `daimon_signal` / `daimon_reap_code`, the channel functions in `src/ipc.cyr`, and the
loop's I/O functions in `src/server.cyr`. The mapping:

| daimon | agnos |
|---|---|
| fork + exec | `spawn_path`#43 (argv line, env blob) |
| signal an agent | `kill`#16 (a pending bit; the agent reads a signalfd) |
| reap | `waitpid`#4 (non-blocking: code, -2 running, -1 not a child) |
| the agent's channel | `chan_op`#97: `CH_MINT`, `CH_ENDOW`, `CH_SEND` / `CH_RECV` |
| /proc readings | `proclist`#99 (+56: CPU ticks, resident pages) |

**One wire format.** An agent writes the same frames on both targets: a 4-byte length, then JSON,
with one reply byte per frame. On agnos the bytes travel in 64-byte records and `_ipc_recv` joins them
back. The only visible difference is a frame limit of 4092 bytes there: an inbox holds 64 records and
drops its oldest when full.

### 2. daimon's own event loop runs on agnos too, polled

Neither sockets nor channels can be waited on with epoll there, and `accept`#57 and `recv`#49 never
block. So each pass accepts, reads every connection still arriving, and serves every channel. It then
yields with `pause`#14, never `sleep_ms`#41, which disables preemption for the whole sleep. Everything
else is the Linux loop's code: slots, deadlines, deferred answers, `agents_tick`. `server_detach` has
no agnos arm, because agnos has no fork for daimon's use. *(Corrected at 2.4.1: that was wrong. agnos
has had `fork`#96 since 1.56.54. See the addendum.)*

*Rejected:* keeping sandhi's sync loop. It retries accept's `EAGAIN` at once and never yields, it has
no tick for reaping or channels, and a silent connection holds it for 30 s. On agnos that is the
peer's receive deadline, because sandhi's `idle_ms` sets an `SO_RCVTIMEO` agnos does not have.

### 3. A gap in agnos is filed with agnos; daimon's interim behaviour is explicit, documented and audited

On 2026-09-23 the operator ruled on the first of these: *"file issue with AGNOS — YOU ACT LIKE THESE
THINGS CAN'T BE CHANGED!"* agnos is changeable, so daimon does not design around a gap as if it were
permanent. Each gap has a filing in the agnos repo (`docs/development/issues/2026-09-23-*.md`, nine
of them), and a stated interim:

| gap | daimon until it is fixed |
|---|---|
| no resource limits | start without them (operator's ruling); `limits_enforced:false`, audit `agent.limits.unenforced` |
| nothing ends a process | a stop asks with SIGTERM; an agent that ignores it stays Stopping; audit `agent.stop.pending` |
| no SIGSTOP/SIGCONT | pause and resume answer 501 |
| argv split on spaces, 127 bytes | 422 for a name with a space or a longer line |
| every fd inherited; `exec_redirect` one fd | `--agent-output capture` refused at startup |
| `sock_listen` has no address | listen on the NIC's address; warn and audit `http.listen.not_loopback` |
| connection ids have no owner | documented exposure (the audit): an agent can interfere with the API |
| `sleep_ms` holds the CPU | `daimon_yield_ms` (`pause`#14) for every wait of daimon's |
| SYNs dropped in interrupt context; no EOF after FIN; 127.0.0.1 dropped | nothing daimon can do; the API is unreachable from the network until fixed |

*Rejected:* refusing to run on agnos until every gap closes. The mapping is testable now, and each
interim is one decision point to revisit when its filing closes.

### 4. Test on the kernel, not only for it

`tests/agnos/run.sh` boots the prebuilt kernel under QEMU and reads a verdict from the serial console.
It runs:
- `src/agent.cyr` directly (`guest.cyr`);
- the real daimon binary, driven over TCP to the guest's own address (`http_client.cyr`). That path
  goes through the kernel's loopback queue, drained in syscall context, so the SYN defect does not
  touch it.

A launcher holds the `/bin/agnsh` slot that kybernet execs. Everything under test runs as a
background process, as daimon would in service.

## Consequences

- An agent on agnos is started, stopped, collected, heard on its channel and measured. It is checked
  on the real kernel, 64 checks.
- daimon's HTTP API on agnos works for local clients on the guest's own address. It is unreachable
  from the network until the SYN filing is fixed, and exposed to other processes on the box until
  the connection-owner filing is.
- The Linux paths are unchanged in behaviour. Benchmarks are within ±3% on the paths 2.4.0 touched.
- Every interim in the table above is a place to come back to when its agnos filing closes.

## Addendum — 2.4.1

**Capacity: `max_agents` stays at 1000 on agnos.** It bounds registrations, and a registration costs
the kernel nothing. What bounds *running* agents is the kernel's tables: 16 process slots and 16
channels for the whole machine, and 32 fds per process. Measured in the guest, 4 processes were live
before the first agent, so 12 agents ran at once. The 13th start was refused by `spawn_path`, and
every slot and channel came back when they stopped. Lowering `max_agents` would not prevent that
refusal. It would only refuse registrations that cost nothing. A start past the tables is refused
cleanly, and the refusal is as precise as agnos allows:
- no free channel (`CH_MINT` answers `-CH_E_FULL`): `AGENT_OP_CAPACITY`, **503**, audited
  `agent.spawn.capacity`, and the agent stays as it was;
- `spawn_path` failed: 500, "its process table is full, or it cannot load the executable". `#43`
  gives the same -1 for both (filed), and `proclist` cannot tell them apart either, because an
  exited, unreaped process holds its slot without being listed.

**daimon keeps its own clock (`daimon_now_ms`).** cyrius's `clock_now_ms` on agnos is
`uptime_us`#95. The kernel calibrates it once, at boot. If it refuses that calibration, #95 answers
-1 for the whole boot, and `clock_now_ms` stands still. A guest booted with QEMU held to 25% of a CPU
did refuse it, and hung in its first 10 ms wait. `daimon_now_ms` falls back to `uptime_ms`#40, the
100 Hz tick, which advances for a background process. Every deadline in daimon reads it. Filed with
agnos (retry or fall back in the kernel) and with cyrius (fall back in `clock_now_ms`).

**Correction: agnos has a fork, so detached calls run there too (2.4.1).** Decision 2 said
`server_detach` has no agnos arm "because agnos has no fork for daimon's use". That was not checked,
and it is wrong: `fork`#96 (1.56.54, fixed 1.56.55) copies the address space and the fd table, and
the peer wraps it as `sys_fork`. Until 2.4.1, an MCP forward and the builtins that wait on another
server (`web_fetch`, `web_search`) ran in place on agnos. They held every other client, every channel
and every stop until their server answered, which could be up to 30 s for each receive that got
nothing. 2.4.1 gives `server_detach` its agnos arm, the Linux design over `fork`#96 and `pipe`#25,
with three differences:
- **The child writes its answer whole.** An agnos pipe is a 4080-byte ring whose write returns short
  when full, and sandhi's `sock_send` is one `sys_write` that ignores a short count. The child names
  its pipe (`http_answer_into_pipe`), and its answer goes through `daimon_write_all`, which waits while
  the loop drains the pipe. The loop relays each detached slot on every pass and reaps the children
  on its tick.
- **A child cannot be killed at the deadline** (AG-4). The client is answered 504 at 60 s as on
  Linux. The SIGKILL is only a request, so the child runs until its upstream gives up.
- **At most 4 children are alive at once** (`SERVE_DETACH_CHILDREN_MAX`). One past that answers 503.
  The machine has 16 process slots, and the agents need them.

**daimon writes its answers in 1 KB pieces on agnos.** A TCP connection's receive ring there is 2048
bytes, and `sock_send`#48 holds the CPU while it waits for room. So one write of more than that to a
process on the same machine never finishes, because the receiver cannot run to drain its ring.
Measured: a `GET /v1/agents` with a 2389-byte body stopped the guest. On agnos, `http_send_response`
writes the same bytes sandhi would, through `daimon_write_all`: at most 1 KB at a time, with a yield
between writes. A client that reads keeps up. One that stops reading can still stop the machine. That,
and `sock_connect`#47 holding the CPU up to ~8 s, is the agnos filing
`2026-09-23-sock-send-and-connect-hold-the-cpu.md`.

**The guest test runs in CI** (the `agnos-guest` job). It uses the released agnos and gnoboot
binaries, downloaded by `tests/agnos/run.sh --release` and pinned by SHA-256 in that script. They are
byte-identical to the builds 2.4.0 was measured on. It runs under QEMU TCG, and every wait in it is a
deadline poll. Measured locally with 2.4.1's final code, all 92 checks pass: unrestricted in 77 s,
with QEMU held to 50% of a CPU in 136 s (the kernel accepted a calibration 2.6× too large there), and
at 25% in 295 s, where the kernel refused its calibration and daimon ran on the tick.

## Addendum — 2.4.2

**A detached call's read waits for the RTC, not for 6000 pauses.** A child's blocking read on a
socket is the stdlib's `_agnos_sock_recv_block` (cyrius, `lib/syscalls_x86_64_agnos.cyr`). It pauses
between polls, and gives up at the RTC's 30 s or after 6000 pauses, which its comment calls "a
hlt-count backstop for when the RTC is unreadable". But a pause halts only when nothing else is
ready. With daimon's loop and its agents ready, 6000 pauses took 1.08 s in the guest. So a server
slower than that was taken for one that had closed, and the call was answered 502. CI's guest test
failed on it, intermittently.

daimon now sets the stdlib's `AGNOS_SOCK_RECV_MAX_SPINS` beyond reach at `serve`'s start, when
`sys_time_unix()` reads (`daimon_agnos_recv_bound`). The RTC's bound then decides, as the stdlib
intends, and detached children inherit the setting. It is the stdlib's own knob, not a copy of its
read, so the fix upstream (filed with cyrius, agnos audit AG-15) replaces it with nothing else to
change. Rejected:
- a read loop of daimon's own for the children: sandhi's client would have to take it as a
  transport, and it would fork the emulation;
- a larger fixed count: a pause lasts anything from under a millisecond to a tick, so no count of
  them is a time.
