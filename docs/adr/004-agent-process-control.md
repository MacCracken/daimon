# ADR-004: Agent Process Control on an Unauthenticated API

**Status**: Accepted. Decision 4 is superseded by [ADR-006](006-own-event-loop.md); see
**Since then** below.
**Date**: 2026-09-22 (2.3.0)
**Context**: 2.3.0 lets HTTP clients start, stop, pause, resume and delete agent processes. daimon
has no authentication until roadmap 2.5.x, and no process-control route existed in the Rust
original to copy. Four decisions follow from putting process control on an API that anyone who can
reach it may call. The evidence behind them is in
[docs/audit/2026-09-22-agent-lifecycle-audit.md](../audit/2026-09-22-agent-lifecycle-audit.md).

**Since then**:
- Decisions 1 and 2 stand.
- Decision 3's guard was extended at 2.3.4. While daimon listens on loopback, a request must name a
  loopback host, and a state change from another site's page is refused on every route.
- Decision 4 is superseded by ADR-006 (2.3.4). A stop is answered when the agent is gone, and the
  loop advances it meanwhile, so it no longer holds other requests (VULN-014).
- The last consequence ended at 2.4.0, when agents started on AGNOS
  ([ADR-007](007-daimon-on-agnos.md)).

## Decisions

### 1. The request never names what runs

A registration names a **type** (System / User / Service, agnostik's `AgentType`). A start runs
`agnos-agent-<type>-agent` from `/usr/lib/agnos/agents` or `/opt/agnos/agents`, else
`/usr/bin/agnos-agent-runner`, which is the Rust original's `find_agent_executable`. The one
override is `serve --agents-dir DIR`, a CLI flag and never a request field.

*Rejected*: an `executable` field in the body. The pre-2.3.0 `agent_start(h, executable)`
anticipated one. On an unauthenticated API that is remote code execution: the shape of Ray's
CVE-2023-48022 and of the payload in Selenium Grid's CVE-2022-28108. *Also rejected*: the Rust
original's third directory, `./agents`, because a cwd-relative search lets whoever controls the
working directory choose what runs.

### 2. The API binds loopback unless told otherwise

`listen_addr` (127.0.0.1) is what the server binds, and `serve --listen ADDR` widens it. Through
2.2.3 daimon bound 0.0.0.0 while its config said 127.0.0.1 (VULN-011).

*Consequence*: clients on other hosts (edge nodes, federation peers) must be enabled explicitly. That
is a behaviour change, called out at the top of the 2.3.0 CHANGELOG entry.

### 3. Agent control refuses browser-originated requests

Start / stop / pause / resume / DELETE answer 403 to any request carrying an `Origin` header.
Browsers attach one to cross-origin requests, including the `text/plain` POST that needs no CORS
preflight, and a DNS-rebinding page sends its own. daimon's clients send none (VULN-012).

*Rejected for now*: applying the guard to every mutating route. It would break any browser-based
consumer of the existing API. It is the cheap step recorded under roadmap 2.5.x, together with a
`Host` allowlist. *Not a substitute for authentication*: the guard stops browsers, not a local
process.

### 4. A stop is synchronous and bounded

SIGTERM, then up to 5 s on a monotonic clock, then SIGKILL and up to 1 s to reap. The request
returns the final status and exit code. The Rust original waited 10 s, but on an async runtime.
daimon's handlers run to completion, so a stop holds every other request (measured: a request sent
during a 5 s stop waited 4.8 s, in both serve modes). 5 s is the hold daimon already accepts from one
slow connection (`SERVE_IDLE_MS`).

*Rejected for now*: 202 Accepted with SIGKILL escalation from a supervisor tick. daimon's sync
loop has no periodic hook, so a stubborn agent would only be killed when some later request
happened to run the sweep. Revisit when the server grows a tick (VULN-014, roadmap).

## Consequences

- The child of every start is set up in one place (`agent_spawn_with_limits`):
  - descriptors closed;
  - stdin `/dev/null`;
  - SIGPIPE reset;
  - rlimits checked;
  - exec failure reported synchronously.
- Every agent route reaps first, so answers reflect processes that exited on their own. The cost is
  ~0.5 µs per running agent (measured).
- AGNOS answers 501 on start until 2.4.x maps `sys_spawn_path` onto the same functions' AGNOS arms.
