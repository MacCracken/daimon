# Daimon Roadmap

> **Scope**: open work only. Shipped work is recorded in [CHANGELOG.md](../../CHANGELOG.md), not here — if an item's text is mostly explaining what was *fixed*, it belongs there instead.
>
> **Severity legend**: **P0** blocking (security / correctness — must-fix before ship) · **P1** high (must-have for the current arc) · **P2** medium (schedule when capacity opens) · **P3 / Low** nice-to-have, no urgency. Upstream-blocker items quote the upstream tracker's own severity.

**Where daimon stands** — `2.2.0`, on cyrius 6.6.6, nine dep pins current. Builds and runs on
**three targets**: x86_64, aarch64 and AGNOS. **306 tests**, 5 fuzz harnesses, 21 benchmarks, all
gates clean. Zero open issue filings. MCP surface: 13 tools — libro audit x5, bote web x2, nein
firewall x6 (mutating half gated shut until 2.5.x). **The 2.2.x arc is underway** — `agent` is
migrated off its mirror; `supervisor` and `ipc` are next.

## The arc to 3.0.0

daimon is an agent *registry* that cannot start an agent. Everything below is that gap and its
prerequisites, sequenced. Each line is a release train, not a single release.

| arc | theme | why it must come after the one above |
|---|---|---|
| **2.2.x** | **Test integrity** — migrate the mirrored suites onto `src/` | It is the net everything else lands in. The lifecycle code has **never been executed**; migrating its tests answers whether it works *before* anything depends on it. |
| **2.3.x** | **Agent lifecycle** — start / stop / signal / reap through the API | The product gap. Needs 2.2.x first: wiring `fork`/`execve`/signals while the suite tests a parallel reimplementation is how two releases already shipped defects green. |
| **2.4.x** | **AGNOS spawn + IPC** — `sys_spawn_path`, `chan_op` capability channels, `sys_proclist` | Nothing to map until a route actually spawns. Unblocks the moment 2.3.x lands. |
| **2.5.x** | **Agent identity + MCP authentication** | Prerequisite for un-gating nein's mutating firewall tools, and for any `claims`-based authorisation. Needs 2.3.x, because identity is per-agent. |
| **3.0.0** | **Per-agent arena isolation** — VULN-007's open half; unlocks multi-tenant hosting, kavach sandboxing, untrusted federation, external MCP callbacks | Major because it changes the allocation model under every agent and flips the gates the P0 below guards. Needs identity (2.5.x) to know what a tenant *is*. |

⚠ The 3.0.0 line is where the security gates open, not where they are first considered. Each `2.x.0`
cut re-evaluates the P0 below.


---

## 2.2.x · P1 — Tests, benches and fuzz harnesses include no `src/` file

`tests/daimon.tcyr`, `tests/daimon.bcyr` and all five `fuzz/*.fcyr` reimplement simplified copies of
the functions they name, so a passing suite has never been a statement about daimon's shipping
source. Two shipped security defects were green under it, each because the mirror never called the
code that was wrong.

**In progress.** `agent` landed at **2.2.0**: `tests/agent.tcyr` covers the real module and its
real dependency chain (33 assertions), and the mirror plus its 20 assertions are gone from
`daimon.tcyr`. It found two defects immediately — `read_vm_rss` could never parse a number (fixed,
live since the port) and `dir_list` enumerates nothing under `/proc/<pid>/fd` (upstream, pinned as
a KNOWN GAP assertion that fails when the stdlib fixes it). Five files now include `src/` directly.

⚠ `agent_next_id` is still mirrored: the screen / scheduler / mcp mirrors call it, so it comes out
with whichever of those migrates first. **Next: `supervisor`, then `ipc`** — the modules 2.3.x
needs.

**One module per bite, suite green at each step** — not one cut-over. Start with the modules the
next arc touches: `agent`, `supervisor`, `ipc`. Expect the migration to surface defects; that is the
point of it.

## 2.3.x · P1 — Wire the agent lifecycle to the API

**daimon is the AGNOS agent orchestrator and it cannot start, stop or signal an agent.** The
process-lifecycle code exists and looks correct, but nothing calls it:

```
agent_spawn_with_limits   0 callers      agent_ipc_bind    0 callers
agent_start               0 callers      ipc_send          0 callers
agent_pause / agent_resume 0 callers     msg_bus_publish   0 callers
agent_stop                0 callers
```

`src/router.cyr` exposes exactly three agent routes — `GET /v1/agents`, `POST /v1/agents`,
`GET /v1/agents/{id}` — and `api_register_agent` calls `agent_handle_new`, which allocates a record.
No route starts a process; there is no start / stop / pause / resume / delete endpoint at all.

This is the product gap, not a cleanup, and it is why several items below are dormant: the AGNOS
spawn/IPC mapping, the VULN-010 rlimit question, and most of the supervisor's value all sit behind
it.

⚠ **Do it after the test-integrity item, not before.** Spawning processes, delivering signals and
reaping children is exactly the class where the mirrored-test problem has already cost two releases;
wiring it while the suite tests a parallel reimplementation would repeat that with `fork`/`execve`
instead of a string copy. Migrating `src/agent.cyr`'s tests first also answers for free whether this
code works at all — **it has never been executed.**

**Sequence**: migrate `agent` + `supervisor` tests → wire start/stop → signals and reaping → IPC.
One bite each, suite green at every step.

## 2.4.x · P2 — AGNOS spawn + IPC mapping

Blocked on the agent-lifecycle item: the surface that needs mapping is the surface nothing calls.
daimon already builds and boots on AGNOS with the same functional surface as the host build. This
unblocks the moment a route spawns a process.

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

nein's firewall admin tools (`nein_allow` / `nein_deny`) are registered and **gated shut** as of
2.1.8 for exactly this reason. Three things unblock together when this lands: un-gating the firewall admin tools
(`_nein_admin_enabled` stops being an operator-trust flag), per-agent authorisation on every other
MCP tool, and a meaningful definition of "tenant" for the 3.0.0 isolation work.

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
