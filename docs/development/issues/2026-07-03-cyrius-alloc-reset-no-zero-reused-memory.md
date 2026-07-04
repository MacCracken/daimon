# cyrius 6.3.43 — `alloc_reset()` rewinds the bump pointer without zeroing; reused first-chunk memory leaks the prior occupant's bytes

**Status:** OPEN upstream (cyrius) — filed 2026-07-03 by daimon (consumer), cyrius 6.3.43. This is the **structural half** of daimon's VULN-007 gate that the consumer *cannot* fix (`lib/alloc.cyr` is vendored stdlib). daimon ships a partial consumer-side mitigation at 1.3.2 (targeted secret-scrubbing of daimon-owned buffers); the reset boundary itself lives entirely in the stdlib and must be fixed here.

**Severity:** **Low → High.** Low for a single-trust-domain consumer (daimon today). High the moment a consumer hands one allocator to two logical owners across a reset — multi-tenant hosting, sandboxing, untrusted federation, or external tool-callback response data sharing the heap. Same bug class as **CVE-2026-34988** (Wasmtime pooling-allocator cross-guest leak) and **CVE-2022-39393**.

**Component:** `lib/alloc.cyr` — `alloc_reset()` (lines 228–236 in the 6.3.43 snapshot).

## Summary

The cyrius bump allocator is forward-only and chunk-based: 256 MB anonymous `mmap` chunks (`MAP_ANONYMOUS | MAP_PRIVATE`), a single pointer bump per allocation, a fresh chunk `mmap`'d on overflow. **New chunks are safe** — Linux zero-fills anonymous pages on first touch, so a forward allocation onto never-before-touched space reads as zero.

The gap is `alloc_reset()`. It rewinds `_heap_base` / `_heap_ptr` / `_heap_end` to the first chunk and sets `_heap_used = 0`, **but does not zero the reclaimed span**. Subsequent allocations reuse those first-chunk addresses while the previous occupant's bytes are still resident. A consumer that lets two logical owners share the allocator across a reset leaks the first owner's data into the second.

`alloc_reset()` is the **only** point the allocator ever hands the same address to a different owner — everywhere else it moves forward onto kernel-zeroed pages. So this one function is the entire reuse channel.

## Root cause

```cyrius
fn alloc_reset(): i64 {
    _alloc_lock_acquire();
    _heap_base = _heap_first_base;
    _heap_ptr  = _heap_first_base;
    _heap_end  = _heap_first_end;
    _heap_used = 0;                 // rewind — but [_heap_first_base, old _heap_ptr) is NOT zeroed
    _alloc_lock_release();
    return 0;
}
```

## Reproduction (conceptual)

```cyrius
var a = alloc(64);
memset(a, 0xAA, 64);      // "sensitive" bytes
alloc_reset();
var b = alloc(64);        // same address as a
// b reads 0xAA… (stale), not 0x00
```

Contrast: without the reset, `b` is a fresh forward address on a kernel-zeroed page and reads clean. The leak is specific to reset-then-reallocate.

## Why the consumer can't fix it

- `lib/alloc.cyr` is **vendored stdlib** — gitignored, repopulated by `cyrius lib sync` / `cyrius deps`. Any consumer edit is wiped on the next resync.
- daimon **never calls `alloc_reset()` itself** (0 hits in `src/*.cyr`). The reset is driven by **sandhi** (also stdlib) between request batches — see daimon `src/server.cyr:187` on sandhi's per-batch arena reset between epoll drains. Both the reset boundary and the allocator are inside the stdlib; the consumer has no seam to interpose on.

## Proposed fix (in cyrius)

Zero-on-reset is the industry-matched baseline for a bump/arena allocator (Wasmtime resets reused slots to a known-zero state; Android init-on-free; glibc `MALLOC_PERTURB_`). Because a bump allocator has no per-object free, the reset boundary is the natural and *cheap* place to carry the guarantee — one bulk operation per reset, not per allocation, so the two-instruction allocation hot path is untouched.

1. **Zero the used span in `alloc_reset()` before rewinding** — `memset(_heap_first_base, 0, _heap_used)`. Smallest change; transparent to every consumer; O(used) per reset (µs-scale for MiB spans), which is negligible against the batch I/O a reset brackets.
2. **Or, for large page-aligned chunks**, `madvise(_heap_first_base, span, MADV_DONTNEED)` to drop the physical pages so the next fault re-serves fresh kernel-zeroed pages — one syscall, zeroing deferred to the kernel (this is Wasmtime's default per-slot reset).
3. If a transparent cost is unacceptable, expose an opt-in `alloc_reset_zeroed()` and have sandhi call it on the request-arena reset. Transparent (option 1/2) is preferred — no consumer should have to know the reset was unsafe.

## Impact / callers

`alloc_reset()` is called by sandhi's per-batch request-arena management. Any consumer relying on batch-boundary reuse is exposed once it hosts more than one trust domain. Fixing it in cyrius closes the reuse channel for **all** downstream consumers at once and unblocks daimon's VULN-007 gate (below).

## References

- **CVE-2026-34988** (GHSA-6wgr-89rj-399p) — Wasmtime pooling-allocator cross-guest linear-memory leak; fixed by resetting a reused slot to a known-zero/guarded state before reuse.
- **CVE-2022-39393** (GHSA-wh6w-3828-g9qf) — related Wasmtime CoW-image-on-slot-reuse leak.
- daimon **VULN-007** — `docs/audit/2026-04-13-security-audit.md`. This upstream fix is the structural remediation the audit gates multi-tenant / sandboxing / untrusted-federation / external-MCP-callback modes behind.
- daimon **1.3.2** — consumer-side mitigation (targeted `secure_zero` scrubbing of daimon-owned sensitive buffers before the reset window). Reduces exposure but cannot cover the stdlib reset boundary; this issue does.
