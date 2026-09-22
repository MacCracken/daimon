# Daimon Roadmap

> **Scope**: open work only. Shipped work is recorded in [CHANGELOG.md](../../CHANGELOG.md), not here.
>
> **Severity legend**: **P0** blocking (security / correctness — must-fix before ship) · **P1** high (must-have for the current arc) · **P2** medium (schedule when capacity opens) · **P3 / Low** nice-to-have, no urgency. Upstream-blocker items quote the upstream tracker's own severity.

**Current status** — `2.1.5` (development). 2.0.0 replaced the in-tree scheduler with samay; 2.1.x has been toolchain-tracking releases (2.1.3 → cyrius 6.6.2 and the `Result`/`Option` value form; 2.1.4 → cyrius 6.6.4; 2.1.5 → cyrius 6.6.6 + all dep pins current, pin-only, no source change). v1.0 criteria are all met. See CHANGELOG for detail.

## Security Gates (trigger-based)

- [ ] **P0 (gated)** — **VULN-007: Bump allocator memory zeroing.** **MUST fix before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted federation, external MCP callbacks (bote). Two halves: (a) the **structural** fix — **CLOSED upstream in cyrius 6.4.1** (zero-on-reset in `alloc_reset()` across all four allocator backends; the filed [issue](issues/2026-07-03-cyrius-alloc-reset-no-zero-reused-memory.md) is RESOLVED), vendored at daimon **1.3.4**. The bump-allocator reuse channel is closed at the source. (b) the **consumer-side** secret-hygiene layer shipped at **1.3.2** (`secure_zero` in `src/secmem.cyr` + memory-store buffer scrubs) as defense-in-depth. **Only open half: per-agent arena isolation** — the hard prerequisite before any multi-tenant / sandbox / untrusted-federation gate flips (zero-on-reset + hygiene close the *reuse/leak* channels, but do not *isolate* trust domains). Severity is **P0 when triggered**, dormant today because no consumer has flipped any of the gating conditions. Re-evaluate at every v1.x.0 cut.

## Correctness / Security (measured, open)

- [ ] **P1** — **RAG-stored chunks alias the reused HTTP request arena — queries return other clients' request bodies.** `POST /v1/rag/ingest` retains `str_sub` *views* (`lib/str.cyr`: "a view, not a copy") of the request body as the stored chunk text and metadata (`src/rag.cyr:140-141`), and `src/server.cyr:210` documents that buffer as a **reused** per-batch arena — so a later request overwrites the stored record and `POST /v1/rag/query` serves it back. Measured 2026-09-22 with a live binary; **reproduced identically on 6.6.4 and 6.6.6 builds of the same tree, so it is not a 2.1.5 regression.** This is a recurrence of the HIGH bug thoth filed in June and daimon fixed at 1.2.5 for the MCP registry — `src/mcp.cyr` carries two ⚠ blocks saying "`str_clone` EVERY FIELD" and calling it "a documented CVE-class bug in this very file"; the sweep never reached `src/rag.cyr` (`grep -c str_clone`: `mcp.cyr` 16, `rag.cyr` 0). Fix is one call site plus a regression test whose pattern already exists in-repo from 1.2.5. The 235-assertion suite cannot see it: `.tcyr` tests drive the store with `str_from` literals over static bytes that are never reused. [Issue](issues/2026-09-22-rag-chunks-alias-the-request-buffer.md).

## Portability (measured, open)

- [ ] **P2** — **`daimon-aarch64` silently loses its per-IP rate limiter and agent rlimits.** Of the nine x86_64 syscall numbers daimon hand-spells (`src/main.cyr:34-42`, `src/server.cyr:11`, `src/agent.cyr:231`), seven are renumbered at runtime by the aarch64 backend's `ESYSXLAT` chain and are fine; `SYS_GETPEERNAME` 52 runs as `fchmod` (succeeds, buffer untouched → `rate_check` allows everyone — VULN-009 off) and `SYS_SETRLIMIT` 160 runs as `uname` (`-EFAULT`, unchecked → agents spawn with no RLIMIT_AS/CPU — VULN-010 off, and no build warning). Fix: delete the five peer-declared globals (that alone restores `getpeername`), an `#ifdef CYRIUS_ARCH_AARCH64` `prlimit64` arm with a checked return, and a `qemu-aarch64` lane asserting a 429 and a child's `/proc/self/limits`. [Issue](issues/2026-09-14-aarch64-binary-issues-x86-syscall-numbers.md). Pre-existing since the port; measured at 2.1.4, **re-verified at 2.1.5** by disassembling the shipped `daimon-aarch64` — the `ESYSXLAT` routed set grew 44 → **60** rows under cyrius 6.6.6 and neither 52 nor 160 is among the sixteen added, so both defects stand unchanged. The upstream half is **CLOSED**: majra 2.9.1 routes `uuid_generate` through the per-target `sys_getrandom`, and the sixth duplicate-symbol warning (`SYS_GETRANDOM` 318) is gone from daimon's aarch64 build.
- [ ] **P2 (daimon-only now — 3 sites)** — **`--agnos` build fails, 3 errors** (was 53 at 2.1.4). **The upstream blocker is CLOSED at 2.1.5**: bote's `dist/bote.deps` sidecar no longer drags the Linux-internal `syscalls_linux_common` into the agnos unit (it does not appear in the build at all under cyrius 6.6.6 + bote 3.3.13), so the Linux peer no longer compiles beside the standalone agnos peer. What remains is exactly daimon's own share — `SYS_EXECVE` (`src/agent.cyr:262`) and `SYS_WAIT4` (`:321`, `:324`), i.e. an agnos spawn arm — plus the scoping decision on whether `dynlib` / `fdlopen` / `tls` / `mmap` / `net` mean anything on agnos. Blocks crab M7/M8. [Issue](issues/2026-09-14-daimon-does-not-build-for-agnos.md).

## Blocked on Upstream Ports

- [ ] **Low (upstream nein)** — **Firewall MCP tools.** nein's Cyrius port has firewall / mesh / nat / netns ported but **no `mcp.cyr`** (the Rust `mcp.rs` is unported), so daimon can't wire firewall control through its MCP surface yet. No active consumer demand; bumps to **P2** when a consumer asks for it.

## Future (v1.4.0+)

Unsequenced; severity is assigned at the v1.4.0 cut once the arc's shape is chosen.

- [ ] jnana integration — grounded knowledge queries backed by verified AGNOS science data
- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Agent migration between nodes
