# Daimon Roadmap

> **Scope**: open work only. Shipped work is recorded in [CHANGELOG.md](../../CHANGELOG.md), not here.
>
> **Severity legend**: **P0** blocking (security / correctness — must-fix before ship) · **P1** high (must-have for the current arc) · **P2** medium (schedule when capacity opens) · **P3 / Low** nice-to-have, no urgency. Upstream-blocker items quote the upstream tracker's own severity.

**Current status** — `2.1.6` (development). 2.0.0 replaced the in-tree scheduler with samay; 2.1.3/2.1.4/2.1.5 were toolchain-tracking releases (cyrius 6.6.2 → 6.6.4 → 6.6.6); 2.1.6 is a security release — the RAG request-buffer aliasing leak and the two aarch64 syscall defects, plus the raw-syscall sweep that closes the class behind both. v1.0 criteria are all met. See CHANGELOG for detail.

## Security Gates (trigger-based)

- [ ] **P0 (gated)** — **VULN-007: Bump allocator memory zeroing.** **MUST fix before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted federation, external MCP callbacks (bote). Two halves: (a) the **structural** fix — **CLOSED upstream in cyrius 6.4.1** (zero-on-reset in `alloc_reset()` across all four allocator backends; the filed [issue](issues/2026-07-03-cyrius-alloc-reset-no-zero-reused-memory.md) is RESOLVED), vendored at daimon **1.3.4**. The bump-allocator reuse channel is closed at the source. (b) the **consumer-side** secret-hygiene layer shipped at **1.3.2** (`secure_zero` in `src/secmem.cyr` + memory-store buffer scrubs) as defense-in-depth. **Only open half: per-agent arena isolation** — the hard prerequisite before any multi-tenant / sandbox / untrusted-federation gate flips (zero-on-reset + hygiene close the *reuse/leak* channels, but do not *isolate* trust domains). Severity is **P0 when triggered**, dormant today because no consumer has flipped any of the gating conditions. Re-evaluate at every v1.x.0 cut.

## Test integrity (measured, open)

- [ ] **P1** — **The test, bench and fuzz suites include no `src/` file.** `tests/daimon.tcyr`, `tests/daimon.bcyr` and all five `fuzz/*.fcyr` reimplement simplified copies of the functions they name, so "235 tests pass" has never been a statement about daimon's shipping source. Measured at 2.1.6, and it is why both of that release's security defects shipped green: the test copy of `rag_ingest_text` never calls `vec_entry_new`/`vindex_insert`, and `bench_rag_ingest_5k` calls only `chunk_text`. The three files added at 2.1.6 (`tests/rag_alias.tcyr`, `tests/syscall_portability.tcyr`, `tests/rag_ingest.bcyr`) include `src/` directly and show it is feasible — `src/*.cyr` are self-contained modules with no `main`. Migrating the existing mirrors is the open work: each mirrored fn must be deleted in favour of the real one, and the collisions resolved module by module. Do it incrementally — one module per bite, suite green at each step — not as one cut-over.

## Portability (measured, open)

- [ ] **P1 — AGNOS is daimon's PRIMARY target and daimon does not build for it.** `cyrius build --agnos` fails at 3 errors. This is not a portability nice-to-have: daimon *is* the AGNOS agent orchestrator and every consumer is an AGNOS agent. **There is no capability gap** — the agnos kernel (`agnos/kernel/core/syscall.cyr`, the canonical source; 1.57.5, 104 dispatch arms) supplies a primitive for everything daimon does. The work is mapping daimon's Linux-shaped OS calls onto agnos-shaped ones, behind `#ifdef CYRIUS_TARGET_AGNOS` — the pattern **7 sibling repos already use** (kavach 7 files, agnoshi 6, crab 2, patra 2, aegis, bote, agnosai). daimon is the outlier.

  | daimon subsystem | Linux today | agnos primitive |
  |---|---|---|
  | HTTP API (8090) | `sys_socket`/`bind`/`listen`/`accept4` | `sys_sock_listen(port)` — bind+listen in one — then `sys_sock_accept`/`sock_recv`/`sock_send`. `lib/net.cyr` already carries 44 agnos arms. |
  | agent IPC (`src/ipc.cyr`) | AF_UNIX socket at a filesystem path | **`chan_op` (#97) capability channels** — `CH_MINT` returns an unnamed **pair**, `CH_ENDOW` arms one end for placement into the next `spawn_path` child. The child inherits the endpoint at spawn: no name, no path, no unlink-before-bind race. The kernel states this design *"deletes the entire unlink-before-bind race class AF_UNIX carries"*. This is a **stronger** model than the Linux path, not a workaround. |
  | agent spawn (`src/agent.cyr`) | `fork` + `execve` + `prlimit64` | `sys_spawn_path` (#43) — one call, and it **does** tokenize argv from the command line. |
  | supervisor `/proc` reads | `/proc/{pid}/status`, `/stat`, `/fd`, `/task` | **agnos has no `/proc`.** `sys_proclist` (#99) returns the live process table as 64-byte records: pid · state · ppid · name[32] · and `+56` split as **cpu ticks (low u32) / rss pages (high u32)** — i.e. exactly the two numbers `read_vm_rss` and `read_cpu_time_ms` parse out of `/proc` today. |

  **Two real constraints, stated rather than hand-waved.** (1) `CH_SEND` caps a payload at **64 bytes** while daimon's `MAX_MESSAGE_SIZE` is **65536**, so bulk IPC needs `sys_shm_create/write/read/free` for the body with chan carrying the control word — a design decision to make, not a blocker. (2) **agnos has no rlimit syscall at all** (`grep -ci rlimit` over the kernel: 0), so VULN-010's per-agent RLIMIT_AS / RLIMIT_CPU has no direct equivalent; whether that becomes a kernel ask, a scheduler-side quota, or a documented non-guarantee on agnos is an open question worth raising with the agnos side. [Issue](issues/2026-09-14-daimon-does-not-build-for-agnos.md).

  ⚠ **Do not read the cyrius peer (`lib/syscalls_x86_64_agnos.cyr`) as authority.** Its own header says the kernel is canonical; it mirrors 1.56.x against a 1.57.5 kernel. Every claim in this item was read from `agnos/kernel/core/syscall.cyr`.

## Blocked on Upstream Ports

- [ ] **Low (upstream nein)** — **Firewall MCP tools.** nein's Cyrius port has firewall / mesh / nat / netns ported but **no `mcp.cyr`** (the Rust `mcp.rs` is unported), so daimon can't wire firewall control through its MCP surface yet. No active consumer demand; bumps to **P2** when a consumer asks for it.

## Future (v1.4.0+)

Unsequenced; severity is assigned at the v1.4.0 cut once the arc's shape is chosen.

- [ ] jnana integration — grounded knowledge queries backed by verified AGNOS science data
- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Agent migration between nodes
