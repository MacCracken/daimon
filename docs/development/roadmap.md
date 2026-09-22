# Daimon Roadmap

> **Scope**: open work only. Shipped work is recorded in [CHANGELOG.md](../../CHANGELOG.md), not here.
>
> **Severity legend**: **P0** blocking (security / correctness — must-fix before ship) · **P1** high (must-have for the current arc) · **P2** medium (schedule when capacity opens) · **P3 / Low** nice-to-have, no urgency. Upstream-blocker items quote the upstream tracker's own severity.

**Current status** — `2.1.4` (development). 2.0.0 replaced the in-tree scheduler with samay; 2.1.x has been toolchain-tracking releases (2.1.3 → cyrius 6.6.2 and the `Result`/`Option` value form; 2.1.4 → cyrius 6.6.4 + all dep pins current). v1.0 criteria are all met. See CHANGELOG for detail.

## Security Gates (trigger-based)

- [ ] **P0 (gated)** — **VULN-007: Bump allocator memory zeroing.** **MUST fix before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted federation, external MCP callbacks (bote). Two halves: (a) the **structural** fix — **CLOSED upstream in cyrius 6.4.1** (zero-on-reset in `alloc_reset()` across all four allocator backends; the filed [issue](issues/2026-07-03-cyrius-alloc-reset-no-zero-reused-memory.md) is RESOLVED), vendored at daimon **1.3.4**. The bump-allocator reuse channel is closed at the source. (b) the **consumer-side** secret-hygiene layer shipped at **1.3.2** (`secure_zero` in `src/secmem.cyr` + memory-store buffer scrubs) as defense-in-depth. **Only open half: per-agent arena isolation** — the hard prerequisite before any multi-tenant / sandbox / untrusted-federation gate flips (zero-on-reset + hygiene close the *reuse/leak* channels, but do not *isolate* trust domains). Severity is **P0 when triggered**, dormant today because no consumer has flipped any of the gating conditions. Re-evaluate at every v1.x.0 cut.

## Portability (measured, open)

- [ ] **P2** — **`daimon-aarch64` silently loses its per-IP rate limiter and agent rlimits.** Of the nine x86_64 syscall numbers daimon hand-spells (`src/main.cyr:34-42`, `src/server.cyr:11`, `src/agent.cyr:231`), seven are renumbered at runtime by the aarch64 backend's `ESYSXLAT` chain and are fine; `SYS_GETPEERNAME` 52 runs as `fchmod` (succeeds, buffer untouched → `rate_check` allows everyone — VULN-009 off) and `SYS_SETRLIMIT` 160 runs as `uname` (`-EFAULT`, unchecked → agents spawn with no RLIMIT_AS/CPU — VULN-010 off, and no build warning). Fix: delete the five peer-declared globals (that alone restores `getpeername`), an `#ifdef CYRIUS_ARCH_AARCH64` `prlimit64` arm with a checked return, and a `qemu-aarch64` lane asserting a 429 and a child's `/proc/self/limits`. [Issue](issues/2026-09-14-aarch64-binary-issues-x86-syscall-numbers.md). Pre-existing since the port; measured at 2.1.4. Upstream half (majra's `SYS_GETRANDOM` 318 / `_SYS_FCHMOD` 91 / raw `syscall(35)`) filed with majra.
- [ ] **P2 (upstream cyrius/bote + 3 daimon sites)** — **`--agnos` build fails, 53 errors.** Root measured at 2.1.4: bote's `dist/bote.deps` sidecar names the Linux-internal `syscalls_linux_common` as a leaf and `cyrius deps` prepends it target-blind, so the Linux peer compiles beside the standalone agnos peer. Not fixable from daimon's manifest. daimon's own share is `src/agent.cyr` `SYS_EXECVE` / `SYS_WAIT4` (an agnos spawn arm) — plus a scoping decision on whether `dynlib` / `fdlopen` / `tls` / `mmap` / `net` mean anything on agnos. Blocks crab M7/M8. [Issue](issues/2026-09-14-daimon-does-not-build-for-agnos.md).

## Blocked on Upstream Ports

- [ ] **Low (upstream nein)** — **Firewall MCP tools.** nein's Cyrius port has firewall / mesh / nat / netns ported but **no `mcp.cyr`** (the Rust `mcp.rs` is unported), so daimon can't wire firewall control through its MCP surface yet. No active consumer demand; bumps to **P2** when a consumer asks for it.

## Future (v1.4.0+)

Unsequenced; severity is assigned at the v1.4.0 cut once the arc's shape is chosen.

- [ ] jnana integration — grounded knowledge queries backed by verified AGNOS science data
- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Agent migration between nodes

## Moving the cyrius pin to 6.6.6

Current pin: `cyrius = "6.6.4"` (`cyrius.cyml`). Two patches forward. Nothing needs to change
first.

The one item that looks like it lands here is **item 9** — `lib/assert.cyr` now transitively
includes `lib/vec.cyr`, so a consumer defining its own `vec_*` at the same arity newly
collides. daimon is the obvious candidate, because `src/vector_store.cyr` defines a `vec_*`
family: `vec_entry_new` (`:28`), `vec_entry_id` (`:39`), `vec_entry_embedding` (`:40`),
`vec_entry_dim` (`:41`), `vec_entry_content` (`:42`), `vec_entry_metadata` (`:43`). **It does
not collide.** `lib/vec.cyr` exports exactly fourteen names — `vec_cap`, `vec_find`, `vec_get`,
`vec_len`, `vec_new`, `vec_new_a`, `vec_pop`, `vec_push`, `vec_push_a`, `vec_remove`,
`vec_select_nth`, `vec_set`, `vec_sort_by`, `vec_truncate` — and every daimon symbol is
`vec_entry_*`, which matches none of them. The prefix is shared; the names are not.

Everything else checked and empty:

- **No Windows exposure** — `CYRIUS_TARGET_AGNOS` is the only `CYRIUS_TARGET_*` in `src/`, CI
  is `ubuntu-latest`, `cross_bins = ["daimon-aarch64"]` with no `windows-*` job — and no
  `O_APPEND` / `O_TRUNC` outside the vendored `lib/`. The single write is `file_write_all` at
  `src/memory.cyr:88`. Item 1 is a non-event here.
- None of item 3's new compile errors have sites: no `async fn`, no `operator` fn, no
  `ret2`/`rethi`, no SIMD intrinsics, no struct declarations in `src/` at all, and no struct-
  or vector-typed parameter or `var` declaration — so item 5's by-value-struct-param deep copy
  is a no-op as well.
- No top-level bare `{` blocks (item 4), no `regression_*` call sites of its own (item 8 — the
  two `include` hits are `lib/regression.cyr` pulling `lib/regression_agnos.cyr`).
- Arity and `: cstring` scans over the repo's own sources: clean.

One thing to watch that is not new in 6.6.6 but interacts with **item 6** (a global redeclared
later now wins everywhere from program start, and a global declared in two co-linked files
with a different type or size is now a compile error): `cyrius.cyml:91` records **227
"duplicate fn (last definition wins)" warnings** between vendored dists, and `:104` a related
`undefined function 'json_v_parse_str'`. Those are duplicate *functions*, not globals, so the
new global rule does not fire on them — but a tree already carrying that many last-definition
warnings is the kind where a duplicated global could hide. A `^var NAME` duplicate scan across
the co-linked set is cheap insurance before the bump.

After bumping, verify: the full `.tcyr` suite per-file, plus one agent lifecycle through the
HTTP API and a vector-store round trip (`src/vector_store.cyr` is the module the `vec_*` note
above concerns).
