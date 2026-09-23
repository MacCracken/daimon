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
no agnos arm, because agnos has no fork for daimon's use.

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
