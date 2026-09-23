# Daimon Roadmap

> **Scope**: open work only. Shipped work is recorded in [CHANGELOG.md](../../CHANGELOG.md), not here — if an item's text is mostly explaining what was *fixed*, it belongs there instead.
>
> **Severity legend**: **P0** blocking (security / correctness — must-fix before ship) · **P1** high (must-have for the current arc) · **P2** medium (schedule when capacity opens) · **P3 / Low** nice-to-have, no urgency. Upstream-blocker items quote the upstream tracker's own severity.

**Where daimon stands** — `2.4.1`, cyrius 6.6.6, nine dep pins current (samay 1.1.3, ai-hwaccel
2.3.27). Builds on **three targets**: x86_64, aarch64 and AGNOS.
- **Agents can be started, stopped, paused, resumed and deleted through the API** (2.3.0), under
  their rlimits, with exit status reported.
- **On AGNOS too** (2.4.0, [ADR-007](../adr/007-daimon-on-agnos.md)): agents are started, stopped,
  collected, heard on their channel and measured with the kernel's own primitives. daimon runs its
  own loop there. `tests/agnos/run.sh` checks this on agnos 1.57.5 under QEMU, in CI on every push
  since 2.4.1. What agnos cannot do yet (limits, ending a process, a loopback-only listener, …) is
  filed with agnos; see 2.4.x.
- **Scheduled tasks can be started and completed** (2.3.1), and an executor can list its node's
  work.
- **Request bodies are read with the typed JSON parser** (2.3.2): strings are decoded, nesting is
  respected, and a body that is not one JSON object, repeats a top-level key or carries U+0000 in a
  top-level string is refused.
- **Agents can send messages to daimon** (2.3.3) on a channel open as their fd 3, and HTTP clients
  can read and send them (2.3.4) ([docs/guides/agent-ipc.md](../guides/agent-ipc.md)).
- **daimon runs its own event loop** (2.3.4, [ADR-006](../adr/006-own-event-loop.md)): one thread,
  one epoll set. A slow client holds only its own connection, a stop waits for its agent and not
  for the server, and agents are collected as they exit. Agents lead their own process groups and
  die with daimon.
- **1043 tests** in 17 suites, every one against its real `src/` module, plus 131 HTTP smoke checks,
  29 benchmarks, 7 fuzz harnesses and the AGNOS guest test's 85 checks, all run by CI. CI also
  verifies the committed `cyrius.lock` (2.4.1).
- The API binds 127.0.0.1 unless told otherwise (`--listen`). While it does, a request must name a
  loopback host, and no route accepts a write from another site's page.
- Zero open issue filings of daimon's own. Twelve agnos filings and one cyrius filing are open
  upstream (2.4.x).

## The arc to 3.0.0

daimon is an agent *registry* that cannot start an agent. Everything below is that gap and its
prerequisites, sequenced. Each line is a release train, not a single release.

| arc | theme | why it must come after the one above |
|---|---|---|
| **2.3.x** | **Agent lifecycle** — start / stop / signal / reap through the API | The product gap. **2.3.0** shipped the process half (start, stop, pause, resume, delete, reaping); **2.3.1** task start/complete; **2.3.2** request-string decoding; **2.3.3** agent channels; **2.3.4** message routes, daimon's own event loop, and the lifecycle follow-ups. What remains is below. |
| **2.4.x** | **AGNOS spawn + IPC** — `sys_spawn_path`, `chan_op` capability channels, `sys_proclist` | Nothing to map until a route actually spawns. **2.4.0** shipped the mapping (spawn, signal, reap, channels, the polled loop, proclist); **2.4.1** the guest test in CI, capacity, and a clock that survives a refused TSC calibration. What remains waits on agnos filings; see below. |
| **2.5.x** | **Agent identity + MCP authentication** | Prerequisite for un-gating nein's mutating firewall tools, and for any `claims`-based authorisation. Needs 2.3.x, because identity is per-agent. |
| **3.0.0** | **Per-agent arena isolation** — VULN-007's open half; unlocks multi-tenant hosting, kavach sandboxing, untrusted federation, external MCP callbacks | Major because it changes the allocation model under every agent and flips the gates the P0 below guards. Needs identity (2.5.x) to know what a tenant *is*. |

⚠ The 3.0.0 line is where the security gates open, not where they are first considered. Each `2.x.0`
cut re-evaluates the P0 below.


---

## 2.3.x · P1 — Wire the agent lifecycle to the API

Shipped in 2.3.0 – 2.3.4: the CHANGELOG entries, ADR-004 – ADR-006 and the addenda to
[docs/audit/2026-09-22-agent-lifecycle-audit.md](../audit/2026-09-22-agent-lifecycle-audit.md).

**Still open in 2.3.x:**
- **Agents that leave their process group (P3).** A child that calls `setsid` is out of reach of a
  stop. A cgroup per agent would hold it on Linux. agnos has no process groups at all; reaching an
  agent's descendants is part of the 2026-09-23 filing on ending a process.
- **One local client can take all 128 connection slots (P3)**, each for up to 30 s. All loopback
  clients share 127.0.0.1, so a per-IP cap would be global; a per-connection rate would bound it.
- **aarch64 RLIMIT_AS is unverified on hardware.** qemu-aarch64 does not apply it: QEMU's user-mode
  `prlimit64` passes RLIMIT_AS / DATA / STACK through as a no-op.

**Sequence**: the 2.4.x items below as their agnos filings land, and 2.5.x. One bite at a time,
suite green at every step.

## 2.4.x · P2 — AGNOS spawn + IPC mapping

Shipped in 2.4.0 and 2.4.1: the CHANGELOG entries, [ADR-007](../adr/007-daimon-on-agnos.md) and
[docs/audit/2026-09-23-agnos-platform-audit.md](../audit/2026-09-23-agnos-platform-audit.md).

**Waiting on agnos** — twelve filings in the agnos repo, `docs/development/issues/2026-09-23-*.md`.
When each closes, daimon's interim changes:

| agnos filing | daimon's interim today | when it closes |
|---|---|---|
| `inbound-tcp-syn-dropped-by-isr-drain` (P1 for daimon) | the API is reachable only from the box itself | nothing to change; re-measure accept from the host |
| `socket-ids-have-no-owner` (P1) | documented exposure (audit AG-1) | nothing to change; close AG-1 |
| `parent-cannot-end-stop-or-continue-a-child` | a stop asks; pause/resume answer 501 | SIGKILL ends the stop; map pause/resume; reach descendants |
| `no-per-process-resource-limits` | agents run without limits, audited | arm the limits at spawn; `limits_enforced:true`; refuse a start whose limits fail |
| `tcp-server-cannot-be-loopback-only` | listen on the NIC's address, warn and audit | listen on 127.0.0.1 by default, as on Linux; per-client rate limiting |
| `child-inherits-every-fd-and-spawn-arms-leak` | `--agent-output capture` refused | capture stdout and stderr on agnos |
| `spawn-path-args-cannot-contain-spaces` | 422 for a name with a space | pass the argv as separate arguments |
| `sleep-ms-holds-the-cpu` | `daimon_yield_ms` (pause) | nothing to change |
| `sock-recv-never-reports-eof-after-peer-fin` | the guest client reads to `Content-Length` | nothing to change in daimon |
| `spawn-path-failure-gives-no-reason` (2.4.1) | a failed spawn is answered "the process table is full, or it cannot load the executable" (500) | answer a full table as capacity (503), like a full channel table |
| `tsc-calibration-refused-stops-the-us-clock` (2.4.1) | `daimon_now_ms` falls back to `uptime_ms`#40 | nothing to change; the fallback stays for older kernels |
| `sock-send-and-connect-hold-the-cpu` (2.4.1, P1 for daimon) | answers are written in 1 KB pieces with a yield between; a local client that stops reading can still stop the machine | one write per answer again (sandhi's path, as on Linux) |

**Waiting on cyrius** — `docs/development/issues/2026-09-23-daimon-agnos-clock-stands-still-when-tsc-calibration-refused.md`
in the cyrius repo: `clock_now_ms` on agnos stands still when the kernel refused its TSC calibration.
daimon's own timing reads `daimon_now_ms`, which falls back; the vendored libraries' reads of
`clock_now_ms` wait on this. When it closes, `daimon_now_ms` can become `clock_now_ms` again.

**daimon's own, on agnos:** nothing open.

2.4.1 settled the two items that were here before: the guest test runs in CI (on the released
kernel, pinned by SHA-256), and capacity was measured and answered. It also brought detached calls
to agnos, which 2.4.0 had wrongly put down to agnos having no fork (audit AG-13). `max_agents` stays, because the
kernel's tables bound running agents, not registrations (the 2.4.1 CHANGELOG).

## 2.5.x · P1 — Agent identity and MCP caller authentication

`POST /v1/mcp/call` has **no caller authentication**: it dispatches on a tool name taken from the
request body. That is why 2.1.8 registered nein's firewall tools with the mutating half
(`nein_allow` / `nein_deny`) **gated shut** — there is nothing to authorise against. bote's `claims`
argument, the seam an identity would arrive through, is a reserved `0` in the 3.x ABI.

⚠ **What 2.2.2, 2.3.0 and 2.3.4 closed, and what they did not.**
- 2.2.2: every route enforces its method, so a state change can no longer be triggered by a
  cross-origin GET (`<img src>`, prefetch — measured, both `/decommission` and `/cancel` were
  reachable that way).
- 2.3.0: the API binds 127.0.0.1 by default (VULN-011), and the agent-control routes answer 403 to
  any request carrying `Origin` (VULN-012).
- 2.3.4: a request must name a loopback host while daimon listens on loopback (DNS rebinding), and a
  state change from another site's page is refused on every route. A page served from loopback may
  still write (VULN-012's remainder).

**Still open**: anything on the host can still do everything, and so can any client on the network
once `--listen` opens it. On agnos, until the connection-owner filing closes, any process on the
box can also act on daimon's connections directly (audit AG-1). Each of those layers stops browsers;
this item is the fix.

nein's firewall admin tools (`nein_allow` / `nein_deny`) are registered and **gated shut** as of
2.1.8 for exactly this reason. Three things unblock together when this lands: un-gating the firewall admin tools
(`_nein_admin_enabled` stops being an operator-trust flag), per-agent authorisation on every other
MCP tool, and a meaningful definition of "tenant" for the 3.0.0 isolation work.

Also waiting on identity:
- **The agent memory API.** The per-agent memory store works since 2.2.3 (it crashed on its first
  call before), but no route reaches it, and one must not until a caller can be tied to an agent —
  otherwise any client reads any agent's memory. Its records also need the `tags` field the Rust
  original had: `list_by_tag` is a substring search over the record today.
- **Who may replace an external MCP registration.** Re-registering a name replaces it, whoever
  registered it first. (Taking a *builtin's* name is refused since 2.2.3.)
- **Who may report a task** (VULN-016, 2.3.1). Any caller can mark any task running, completed or
  failed. Completing a task early returns its capacity while the work may still run. Only the
  task's executor should be able to.

Needs 2.3.x first — identity is per-agent, and there are no agents until the lifecycle exists.

## 3.0.0 · P0 (gated, dormant) — VULN-007: per-agent arena isolation

**MUST be resolved before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted
federation, or external MCP callbacks (bote). Dormant today because no consumer has flipped any of
those; **P0 the moment one does.** Re-evaluate at every `2.x.0` cut.

Zero-on-reset and secret hygiene close the *reuse* and *leak* channels but do not **isolate trust
domains** — one bump allocator still backs every agent. Isolation is the open half and the hard
prerequisite. (Both shipped halves are in the CHANGELOG.)

## Beyond the arc (unsequenced)

Severity assigned when the arc's shape is chosen.

- [ ] jnana integration — grounded knowledge queries backed by verified AGNOS science data
- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Agent migration between nodes
