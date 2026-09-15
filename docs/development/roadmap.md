# Daimon Roadmap

> **Scope**: open work only. Shipped work is recorded in [CHANGELOG.md](../../CHANGELOG.md), not here.
>
> **Severity legend**: **P0** blocking (security / correctness — must-fix before ship) · **P1** high (must-have for the current arc) · **P2** medium (schedule when capacity opens) · **P3 / Low** nice-to-have, no urgency. Upstream-blocker items quote the upstream tracker's own severity.

**Current status** — `2.1.4` (development). 2.0.0 replaced the in-tree scheduler with samay; 2.1.x has been toolchain-tracking releases (2.1.3 → cyrius 6.6.2 and the `Result`/`Option` value form; 2.1.4 → cyrius 6.6.4 + all dep pins current). v1.0 criteria are all met. See CHANGELOG for detail.

## Security Gates (trigger-based)

- [ ] **P0 (gated)** — **VULN-007: Bump allocator memory zeroing.** **MUST fix before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted federation, external MCP callbacks (bote). Two halves: (a) the **structural** fix — **CLOSED upstream in cyrius 6.4.1** (zero-on-reset in `alloc_reset()` across all four allocator backends; the filed [issue](issues/2026-07-03-cyrius-alloc-reset-no-zero-reused-memory.md) is RESOLVED), vendored at daimon **1.3.4**. The bump-allocator reuse channel is closed at the source. (b) the **consumer-side** secret-hygiene layer shipped at **1.3.2** (`secure_zero` in `src/secmem.cyr` + memory-store buffer scrubs) as defense-in-depth. **Only open half: per-agent arena isolation** — the hard prerequisite before any multi-tenant / sandbox / untrusted-federation gate flips (zero-on-reset + hygiene close the *reuse/leak* channels, but do not *isolate* trust domains). Severity is **P0 when triggered**, dormant today because no consumer has flipped any of the gating conditions. Re-evaluate at every v1.x.0 cut.

## Portability (measured, open)

- [ ] **P1** — **`daimon-aarch64` issues x86_64 syscall numbers.** `src/main.cyr:34-42`, `src/server.cyr:11`, `src/agent.cyr:231` define `SYS_SOCKET` / `SYS_CONNECT` / `SYS_BIND` / `SYS_LISTEN` / `SYS_GETPEERNAME` / `SYS_ACCEPT` / `SYS_GETSOCKOPT` / `SYS_RENAME` / `SYS_SETRLIMIT` as x86_64 globals that win over the aarch64 peer's values (last definition wins), so the shipped aarch64 release asset cannot bind a socket. CI only checks the ELF arch. Five are a pure deletion (the stdlib defines them on both Linux peers); four need `sys_*` wrappers or an `#ifdef` arm; the lane should fail on `duplicate symbol 'SYS_`. [Issue](issues/2026-09-14-aarch64-binary-issues-x86-syscall-numbers.md). Pre-existing since the port; measured at 2.1.4.
- [ ] **P2 (upstream cyrius/bote + 3 daimon sites)** — **`--agnos` build fails, 53 errors.** Root measured at 2.1.4: bote's `dist/bote.deps` sidecar names the Linux-internal `syscalls_linux_common` as a leaf and `cyrius deps` prepends it target-blind, so the Linux peer compiles beside the standalone agnos peer. Not fixable from daimon's manifest. daimon's own share is `src/agent.cyr` `SYS_EXECVE` / `SYS_WAIT4` (an agnos spawn arm) — plus a scoping decision on whether `dynlib` / `fdlopen` / `tls` / `mmap` / `net` mean anything on agnos. Blocks crab M7/M8. [Issue](issues/2026-09-14-daimon-does-not-build-for-agnos.md).

## Blocked on Upstream Ports

- [ ] **Low (upstream nein)** — **Firewall MCP tools.** nein's Cyrius port has firewall / mesh / nat / netns ported but **no `mcp.cyr`** (the Rust `mcp.rs` is unported), so daimon can't wire firewall control through its MCP surface yet. No active consumer demand; bumps to **P2** when a consumer asks for it.

## Future (v1.4.0+)

Unsequenced; severity is assigned at the v1.4.0 cut once the arc's shape is chosen.

- [ ] jnana integration — grounded knowledge queries backed by verified AGNOS science data
- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Agent migration between nodes
