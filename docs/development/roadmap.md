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

- [ ] **P2 (daimon-only — agnos scoping decision)** — **`--agnos` build fails, 3 errors.** The upstream blocker closed at 2.1.5 (bote's sidecar no longer drags `syscalls_linux_common` into the agnos unit). At 2.1.6 the remaining errors changed cause: the raw-syscall sweep resolved the undefined `SYS_EXECVE` / `SYS_WAIT4`, and what surfaces now is that agnos's peer declares **different arities for the same wrapper names** — `sys_waitpid(pid)` (1 arg, not 3), `sys_rename(old, oldlen, new, newlen)` (4 args, not 2) — and defines no `sys_socket` / `sys_bind` / `sys_listen` / `sys_accept4` / `sys_execve` / `sys_pidfd_open` at all. Strictly better than 2.1.5, where `src/memory.cyr` compiled against daimon's `SYS_RENAME = 82` while agnos's `rename` is **31** — a silent wrong-syscall, now a loud compile error. Needs an agnos spawn/socket arm plus the scoping decision on whether `dynlib` / `fdlopen` / `tls` / `mmap` / `net` mean anything there. Blocks crab M7/M8. [Issue](issues/2026-09-14-daimon-does-not-build-for-agnos.md).

## Blocked on Upstream Ports

- [ ] **Low (upstream nein)** — **Firewall MCP tools.** nein's Cyrius port has firewall / mesh / nat / netns ported but **no `mcp.cyr`** (the Rust `mcp.rs` is unported), so daimon can't wire firewall control through its MCP surface yet. No active consumer demand; bumps to **P2** when a consumer asks for it.

## Future (v1.4.0+)

Unsequenced; severity is assigned at the v1.4.0 cut once the arc's shape is chosen.

- [ ] jnana integration — grounded knowledge queries backed by verified AGNOS science data
- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Agent migration between nodes
