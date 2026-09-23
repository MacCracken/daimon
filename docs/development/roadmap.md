# Daimon Roadmap

> **Scope**: open work only. Shipped work is recorded in [CHANGELOG.md](../../CHANGELOG.md), not here — if an item's text is mostly explaining what was *fixed*, it belongs there instead.
>
> **Severity legend**: **P0** blocking (security / correctness — must-fix before ship) · **P1** high (must-have for the current arc) · **P2** medium (schedule when capacity opens) · **P3 / Low** nice-to-have, no urgency. Upstream-blocker items quote the upstream tracker's own severity.

**Where daimon stands** — `2.3.4`, cyrius 6.6.6, nine dep pins current (samay
1.1.3). Builds on **three targets**: x86_64, aarch64 and AGNOS.
- **Agents can be started, stopped, paused, resumed and deleted through the API** (2.3.0), under
  their rlimits, with exit status reported. On AGNOS a start answers 501 until 2.4.x.
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
- **1033 tests** in 17 suites, every one against its real `src/` module, plus 130 HTTP smoke checks,
  29 benchmarks and 7 fuzz harnesses, all run by CI.
- The API binds 127.0.0.1 unless told otherwise (`--listen`). While it does, a request must name a
  loopback host, and no route accepts a write from another site's page.
- Zero open issue filings.

## The arc to 3.0.0

daimon is an agent *registry* that cannot start an agent. Everything below is that gap and its
prerequisites, sequenced. Each line is a release train, not a single release.

| arc | theme | why it must come after the one above |
|---|---|---|
| **2.3.x** | **Agent lifecycle** — start / stop / signal / reap through the API | The product gap. **2.3.0** shipped the process half (start, stop, pause, resume, delete, reaping); **2.3.1** task start/complete; **2.3.2** request-string decoding; **2.3.3** agent channels; **2.3.4** message routes, daimon's own event loop, and the lifecycle follow-ups. What remains is below. |
| **2.4.x** | **AGNOS spawn + IPC** — `sys_spawn_path`, `chan_op` capability channels, `sys_proclist` | Nothing to map until a route actually spawns. Unblocks the moment 2.3.x lands. |
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
  stop. A cgroup per agent would hold it, but that is Linux-specific; decide with 2.4.x.
- **One local client can take all 128 connection slots (P3)**, each for up to 30 s. All loopback
  clients share 127.0.0.1, so a per-IP cap would be global; a per-connection rate would bound it.
- **aarch64 RLIMIT_AS is unverified on hardware.** qemu-aarch64 does not apply it: QEMU's user-mode
  `prlimit64` passes RLIMIT_AS / DATA / STACK through as a no-op.

**Sequence**: 2.4.x next. One bite at a time, suite green at every step.

## 2.4.x · P2 — AGNOS spawn + IPC mapping

**Unblocked by 2.3.0**: a route now spawns a process. On AGNOS, `POST /v1/agents/{id}/start`
answers 501, because `agent_spawn_with_limits`, `daimon_signal` and `daimon_reap_code` have AGNOS
arms that refuse. Those arms are the mapping targets: spawn, signal and reap. `daimon_reap_code`'s arm
already passes agnos's #4 through.

Reference for when it does — every row read from `agnos/kernel/core/syscall.cyr`:

| subsystem | agnos primitive | constraint |
|---|---|---|
| exec | `sys_spawn_path` (#43) — one call; **does** tokenize argv from the command line | no fork/exec split |
| IPC | `chan_op` (#97) — `CH_MINT` returns an unnamed **pair**, `CH_ENDOW` places one end into the next spawned child. The kernel notes this *"deletes the entire unlink-before-bind race class AF_UNIX carries"* — a stronger model than the Linux path, not a workaround | **`CH_SEND` caps a payload at 64 bytes** vs daimon's `MAX_MESSAGE_SIZE` 65536, so bulk bodies need `sys_shm_create/write/read/free` with the channel carrying the control word |
| agent rlimits | — | ⛔ **agnos has no rlimit syscall at all** (`grep -ci rlimit` over the kernel: 0). VULN-010 has no direct equivalent. Whether that becomes a kernel ask, a scheduler-side quota, or a documented non-guarantee is **a question for the agnos side** — raise it there rather than assume it in either direction. |
| supervisor | `sys_proclist` (#99) — 64-byte records: pid · state · ppid · name[32] · `+56` split as **cpu ticks (low u32) / rss pages (high u32)** | agnos has no `/proc`; those are the same two numbers `read_vm_rss` / `read_cpu_time_ms` parse today |

⚠ Read `agnos/kernel/core/syscall.cyr` for any of this, **not** `lib/syscalls_x86_64_agnos.cyr`. The
cyrius peer's own header records that a doc→peer→doc citation loop once let a wrong number verify
itself, and states the kernel is canonical; it mirrors an older kernel.

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
once `--listen` opens it. Each of those layers stops browsers; this item is the fix.

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
