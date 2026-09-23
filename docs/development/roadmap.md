# Daimon Roadmap

> **Scope**: open work only. Shipped work is recorded in [CHANGELOG.md](../../CHANGELOG.md), not here — if an item's text is mostly explaining what was *fixed*, it belongs there instead.
>
> **Severity legend**: **P0** blocking (security / correctness — must-fix before ship) · **P1** high (must-have for the current arc) · **P2** medium (schedule when capacity opens) · **P3 / Low** nice-to-have, no urgency. Upstream-blocker items quote the upstream tracker's own severity.

**Where daimon stands** — `2.3.0`, cyrius 6.6.6, nine dep pins current (samay
1.1.3). Builds on **three targets**: x86_64, aarch64 and AGNOS. **Agents can be started, stopped,
paused, resumed and deleted through the API** (2.3.0), under their rlimits, with exit status
reported. On AGNOS a start answers 501 until 2.4.x. **741 tests** in 16 suites, every one against
its real `src/` module, plus 43 HTTP smoke checks, 24 benchmarks and 6 fuzz harnesses, all run by
CI. The API binds 127.0.0.1 unless told otherwise (`--listen`), and agent control refuses
browser-originated requests. Zero open issue filings.

## The arc to 3.0.0

daimon is an agent *registry* that cannot start an agent. Everything below is that gap and its
prerequisites, sequenced. Each line is a release train, not a single release.

| arc | theme | why it must come after the one above |
|---|---|---|
| **2.3.x** | **Agent lifecycle** — start / stop / signal / reap through the API | The product gap. **2.3.0 shipped the process half** (start, stop, pause, resume, delete, reaping); task start/complete and IPC remain. |
| **2.4.x** | **AGNOS spawn + IPC** — `sys_spawn_path`, `chan_op` capability channels, `sys_proclist` | Nothing to map until a route actually spawns. Unblocks the moment 2.3.x lands. |
| **2.5.x** | **Agent identity + MCP authentication** | Prerequisite for un-gating nein's mutating firewall tools, and for any `claims`-based authorisation. Needs 2.3.x, because identity is per-agent. |
| **3.0.0** | **Per-agent arena isolation** — VULN-007's open half; unlocks multi-tenant hosting, kavach sandboxing, untrusted federation, external MCP callbacks | Major because it changes the allocation model under every agent and flips the gates the P0 below guards. Needs identity (2.5.x) to know what a tenant *is*. |

⚠ The 3.0.0 line is where the security gates open, not where they are first considered. Each `2.x.0`
cut re-evaluates the P0 below.


---

## 2.3.x · P1 — Wire the agent lifecycle to the API

**2.3.0 did the process half** (CHANGELOG 2.3.0,
[docs/audit/2026-09-22-agent-lifecycle-audit.md](../audit/2026-09-22-agent-lifecycle-audit.md)):
- `POST /v1/agents/{id}/start|stop|pause|resume` and `DELETE /v1/agents/{id}`;
- agents chosen by type, never by request;
- the child's descriptors, stdin, SIGPIPE and rlimits set up;
- a grace-period stop;
- reaping with exit status;
- `max_agents` enforced;
- the API bound to 127.0.0.1;
- agent control closed to browsers.

**Next — task start and complete.** samay places tasks on nodes, but nothing moves one from
SCHEDULED to RUNNING, so through the API every task stops at SCHEDULED. samay already has what the
routes need:
- `scheduled_task_transition(task, TASK_RUNNING)` accepts SCHEDULED → RUNNING;
- `task_scheduler_complete_task(s, id, final_status)` finishes a RUNNING task and returns its node
  reservation.

daimon calls neither. Whatever executes an agent on a node is what should report a task running and
done.

**Then — IPC.** `agent_ipc_bind`, `agent_ipc_send` and `msg_bus_publish` still have no caller.
Three defects were found when the socket code first ran (2.2.3) and deliberately left for this step:
- the SO_PEERCRED check (VULN-006) **fails open**: if `getsockopt` fails, the peer is not checked;
- a message cut short by its sender is queued and ACKed as if whole;
- `agent_ipc_accept_one` reads with no timeout, so one silent peer holds the accept loop.

**Lifecycle follow-ups, recorded by the 2.3.0 audit:**
- **A stop holds the server** (VULN-014, P2): up to ~6 s, in both serve modes. Measured: a request
  sent during a 5 s stop waited 4.8 s. A non-blocking stop (202, then SIGKILL from a supervisor
  tick) needs a periodic hook the sync loop lacks.
- **An agent's own children are not signalled.** A stop signals the agent's process, not a process
  group.
- **Agents outlive a daimon crash**: there is no parent-death signal, and the registry is in memory.
  Decide whether a restarted daimon adopts, kills or ignores them.
- **stdout and stderr are inherited, not captured.** The supervisor's `OutputCapture` is not wired.
- **The environment is inherited** (VULN-015, Rust parity). An allowlist per agent type would keep
  daimon's secrets out of agents.
- **aarch64 RLIMIT_AS is unverified on hardware.** qemu-aarch64 does not apply it: QEMU's user-mode
  `prlimit64` passes RLIMIT_AS / DATA / STACK through as a no-op.

**Sequence**: task start/complete → IPC. One bite each, suite green at every step.

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

⚠ **What 2.2.2 and 2.3.0 did and did not close.**
- 2.2.2: every route enforces its method, so a state change can no longer be triggered by a
  cross-origin GET (`<img src>`, prefetch — measured, both `/decommission` and `/cancel` were
  reachable that way).
- 2.3.0: the API binds 127.0.0.1 by default (VULN-011), and the agent-control routes answer 403 to
  any request carrying `Origin` (VULN-012).

**Still open** — a cross-origin `text/plain` POST, which needs no preflight, still drives:
- MCP tool registration, RAG ingest, edge decommission, and scheduler submit and cancel;
- and, with no `Host` allowlist, a DNS-rebinding page can read every GET response.

Anything on the host can still do everything. Each of those layers is a floor; this item is the
fix. **Cheaper steps before it:** a `Host` allowlist (loopback names plus a configured list) and the
Origin guard on every mutating route. Both would break a browser-based consumer, if one exists.

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

Needs 2.3.x first — identity is per-agent, and there are no agents until the lifecycle exists.

## 3.0.0 · P0 (gated, dormant) — VULN-007: per-agent arena isolation

**MUST be resolved before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted
federation, or external MCP callbacks (bote). Dormant today because no consumer has flipped any of
those; **P0 the moment one does.** Re-evaluate at every `2.x.0` cut.

Zero-on-reset and secret hygiene close the *reuse* and *leak* channels but do not **isolate trust
domains** — one bump allocator still backs every agent. Isolation is the open half and the hard
prerequisite. (Both shipped halves are in the CHANGELOG.)

## P3 — Edge registration takes no capabilities

`POST /v1/edge/nodes` records every node as `x86_64`, 4 cores, 4096 MB whatever it is, so placement
and stats describe a fleet that does not exist. Read `arch` / `cpu_cores` / `memory_mb` from the
body (validated). Its duplicate-name check is also a linear scan per registration — bounded by
`max_nodes` (1,000), so not urgent.

## Beyond the arc (unsequenced)

Severity assigned when the arc's shape is chosen.

- [ ] jnana integration — grounded knowledge queries backed by verified AGNOS science data
- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Agent migration between nodes
