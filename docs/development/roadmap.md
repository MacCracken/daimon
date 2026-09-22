# Daimon Roadmap

> **Scope**: open work only. Shipped work is recorded in [CHANGELOG.md](../../CHANGELOG.md), not here — if an item's text is mostly explaining what was *fixed*, it belongs there instead.
>
> **Severity legend**: **P0** blocking (security / correctness — must-fix before ship) · **P1** high (must-have for the current arc) · **P2** medium (schedule when capacity opens) · **P3 / Low** nice-to-have, no urgency. Upstream-blocker items quote the upstream tracker's own severity.

**Where daimon stands** — `2.1.7`, on cyrius 6.6.6, all eight dep pins current. Builds and runs on
**three targets**: x86_64, aarch64 and AGNOS. 293 tests, 5 fuzz harnesses, 21 benchmarks, all gates
clean. v1.0 criteria were met at 1.0.0 and are not re-litigated here. **Zero open issue filings** —
`docs/development/issues/` is empty; all 8 sit in `issues/archive/`.

**The next arc is 2.2, and it has one shape**: daimon is an agent *registry* that cannot start an
agent. The two P1s below are that arc, in that order — test integrity first, because it is the net
everything else has to land in.

---

## P1 — Wire the agent lifecycle to the API

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

## P1 — Tests, benches and fuzz harnesses include no `src/` file

`tests/daimon.tcyr`, `tests/daimon.bcyr` and all five `fuzz/*.fcyr` reimplement simplified copies of
the functions they name, so a passing suite has never been a statement about daimon's shipping
source. Two shipped security defects were green under it, each because the mirror never called the
code that was wrong.

**The path is proven.** Four files now include `src/` directly — `tests/rag_alias.tcyr`,
`tests/syscall_portability.tcyr`, `tests/version_sync.tcyr`, `tests/rag_ingest.bcyr` — and
`src/*.cyr` are self-contained modules with no `main`, so inclusion works. The remaining work is
deleting each mirrored function in favour of the real one and resolving the collisions.

**One module per bite, suite green at each step** — not one cut-over. Start with the modules the
next arc touches: `agent`, `supervisor`, `ipc`. Expect the migration to surface defects; that is the
point of it.

## P0 (gated, dormant) — VULN-007: per-agent arena isolation

**MUST be resolved before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted
federation, or external MCP callbacks (bote). Dormant today because no consumer has flipped any of
those; **P0 the moment one does.** Re-evaluate at every `2.x.0` cut.

Zero-on-reset and secret hygiene close the *reuse* and *leak* channels but do not **isolate trust
domains** — one bump allocator still backs every agent. Isolation is the open half and the hard
prerequisite. (Both shipped halves are in the CHANGELOG.)

## P2 — AGNOS spawn + IPC mapping

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

## Blocked on upstream

- [ ] **Low (upstream nein)** — **Firewall MCP tools.** nein 1.6.11 has firewall / mesh / nat / netns ported but ships only `src/main.cyr` — no `mcp.cyr` (the Rust `mcp.rs` is unported) — so daimon cannot wire firewall control through its MCP surface. Verified at 2.1.7. No active consumer demand; bumps to **P2** when a consumer asks.
- [ ] **Low (upstream sandhi)** — **`serve_async` collapse into `sandhi_server_run_opts`.** `sandhi_server_options_max_conns` is accepted but not honoured — `lib/sandhi.cyr` still reads *"reserved for 0.8.0+"*. No security impact. Closes when upstream wires worker-pool or epoll-cooperative enforcement.

## Future (2.2+, unsequenced)

Severity assigned when the arc's shape is chosen.

- [ ] jnana integration — grounded knowledge queries backed by verified AGNOS science data
- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Agent migration between nodes
