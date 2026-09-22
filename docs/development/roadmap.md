# Daimon Roadmap

> **Scope**: open work only. Shipped work is recorded in [CHANGELOG.md](../../CHANGELOG.md), not here.
>
> **Severity legend**: **P0** blocking (security / correctness — must-fix before ship) · **P1** high (must-have for the current arc) · **P2** medium (schedule when capacity opens) · **P3 / Low** nice-to-have, no urgency. Upstream-blocker items quote the upstream tracker's own severity.

**Current status** — `2.1.7` (development). 2.0.0 replaced the in-tree scheduler with samay; 2.1.3–2.1.5 were toolchain-tracking releases (cyrius 6.6.2 → 6.6.6); 2.1.6 was a security release (RAG request-buffer aliasing + the two aarch64 syscall defects + the raw-syscall sweep); **2.1.7 is the AGNOS port** — daimon builds for `--agnos` for the first time — plus an issue-tracker close-out (seven filings archived, one open). v1.0 criteria are all met. See CHANGELOG for detail.

## Security Gates (trigger-based)

- [ ] **P0 (gated)** — **VULN-007: Bump allocator memory zeroing.** **MUST fix before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted federation, external MCP callbacks (bote). Two halves: (a) the **structural** fix — **CLOSED upstream in cyrius 6.4.1** (zero-on-reset in `alloc_reset()` across all four allocator backends; the filed [issue](issues/archive/2026-07-03-cyrius-alloc-reset-no-zero-reused-memory.md) is RESOLVED — re-verified at the 6.6.6 pin in 2.1.7 and archived), vendored at daimon **1.3.4**. The bump-allocator reuse channel is closed at the source. (b) the **consumer-side** secret-hygiene layer shipped at **1.3.2** (`secure_zero` in `src/secmem.cyr` + memory-store buffer scrubs) as defense-in-depth. **Only open half: per-agent arena isolation** — the hard prerequisite before any multi-tenant / sandbox / untrusted-federation gate flips (zero-on-reset + hygiene close the *reuse/leak* channels, but do not *isolate* trust domains). Severity is **P0 when triggered**, dormant today because no consumer has flipped any of the gating conditions. Re-evaluate at every v1.x.0 cut.

## Test integrity (measured, open)

- [ ] **P1** — **The test, bench and fuzz suites include no `src/` file.** `tests/daimon.tcyr`, `tests/daimon.bcyr` and all five `fuzz/*.fcyr` reimplement simplified copies of the functions they name, so "235 tests pass" has never been a statement about daimon's shipping source. Measured at 2.1.6, and it is why both of that release's security defects shipped green: the test copy of `rag_ingest_text` never calls `vec_entry_new`/`vindex_insert`, and `bench_rag_ingest_5k` calls only `chunk_text`. The three files added at 2.1.6 (`tests/rag_alias.tcyr`, `tests/syscall_portability.tcyr`, `tests/rag_ingest.bcyr`) include `src/` directly and show it is feasible — `src/*.cyr` are self-contained modules with no `main`. Migrating the existing mirrors is the open work: each mirrored fn must be deleted in favour of the real one, and the collisions resolved module by module. Do it incrementally — one module per bite, suite green at each step — not as one cut-over.

## Portability (measured, open)

- [ ] **P2 — AGNOS spawn + IPC, when the API wires them.** daimon **builds and boots on AGNOS** as of 2.1.7 (verified in ring 3 on a production kernel under QEMU — see the [archived filing](issues/archive/2026-09-14-daimon-does-not-build-for-agnos.md)), with the same functional surface as the host build. What is *not* mapped is the process-spawn and IPC surface — and it is unmapped because **it is unreachable on every target**: `agent_spawn_with_limits`, `agent_start`, `agent_ipc_bind`, `ipc_send` and `msg_bus_publish` have zero callers, and the HTTP agent-create handler registers a record without spawning a process. This item unblocks the moment that changes.

  | subsystem | agnos primitive | constraint |
  |---|---|---|
  | exec | `sys_spawn_path` (#43) — one call; **does** tokenize argv from the command line | no fork/exec split |
  | IPC | `chan_op` (#97) — `CH_MINT` returns an unnamed **pair**, `CH_ENDOW` places one end into the next spawned child. The kernel notes this *"deletes the entire unlink-before-bind race class AF_UNIX carries"* — a stronger model than the Linux path, not a workaround | **`CH_SEND` caps a payload at 64 bytes** vs daimon's `MAX_MESSAGE_SIZE` 65536, so bulk bodies need `sys_shm_create/write/read/free` with the channel carrying the control word |
  | agent rlimits | — | ⛔ **agnos has no rlimit syscall at all** (`grep -ci rlimit` over the kernel: 0). VULN-010 has no direct equivalent. Whether that becomes a kernel ask, a scheduler-side quota, or a documented non-guarantee on agnos is **a question for the agnos side** and should be raised there rather than assumed in either direction. |
  | supervisor | `sys_proclist` (#99) — 64-byte records: pid · state · ppid · name[32] · `+56` split as **cpu ticks (low u32) / rss pages (high u32)** | agnos has no `/proc`; those are the same two numbers `read_vm_rss` / `read_cpu_time_ms` parse today |

  ⚠ Read `agnos/kernel/core/syscall.cyr` for any of this, **not** `lib/syscalls_x86_64_agnos.cyr`. The cyrius peer's own header records that a doc→peer→doc citation loop once let a wrong number verify itself, and states the kernel is canonical; it mirrors an older kernel.

## Blocked on Upstream Ports

- [ ] **Low (upstream nein)** — **Firewall MCP tools.** nein's Cyrius port has firewall / mesh / nat / netns ported but **no `mcp.cyr`** (the Rust `mcp.rs` is unported), so daimon can't wire firewall control through its MCP surface yet. No active consumer demand; bumps to **P2** when a consumer asks for it.

## Future (v1.4.0+)

Unsequenced; severity is assigned at the v1.4.0 cut once the arc's shape is chosen.

- [ ] jnana integration — grounded knowledge queries backed by verified AGNOS science data
- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Agent migration between nodes
