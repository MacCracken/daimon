# cyrius 6.1.40 — address-taken fixed local array under-reserves static backing, corrupts adjacent literal

**Status:** RESOLVED upstream in **cyrius 6.2.1**; adopted daimon-side in 1.2.7
(pin → 6.2.2). The fix was a **language change, not a silent codegen patch** —
see the cyrius 6.2.1 CHANGELOG ("element-typed arrays `var a: T[N]` + daimon-class
slot-array sweep"). Bare `var a[N]` now has explicit per-scope semantics (N
**bytes** in a fn, N i64 slots at top level); the unambiguous slot form is
`var a: i64[N]`, which reserves the full `N*8` bytes identically in any scope.
daimon converted every address-taken slot/multi-byte local array to a sized
element-typed array (`argv_buf: i64[4]`, two `status_names: i64[N]`, plus
`status_buf: i32[1]` / `cred_len: u32[1]` / `len_buf` + `hdr: u8[4]`); see
CHANGELOG [1.2.7]. The `ip_to_cstr` inline workaround is retained (allocates no
array; clearest for that hot path).

> **Note (2026-06-12):** an interim re-test claimed this "still reproduces under
> 6.2.0/6.2.1" — that was a **test error**: the reproducer used bare `var parts[4]`,
> which under the 6.2.1 language is *correctly* a 4-byte buffer, so a 32-byte slot
> write overruns *by design*. Re-tested with `var parts: i64[4]` → exit 32 (clean)
> under 6.2.2. Read the upstream CHANGELOG before re-testing a "fixed" footgun.

**Severity:** HIGH (upstream cyrius) — silent static-memory corruption; in daimon
it manifested as *every* HTTP route returning 404
**Component:** cyrius compiler (`cycc` 6.1.40, also reproduced under 6.1.39) —
storage allocation for address-taken fixed-size local arrays. Fixed in 6.2.1 by
the element-typed-array language feature.
**Filed by:** daimon (consumer)

## Summary

When a fixed-size local array `var a[N]` has its address taken (`&a`) and that
address escapes the function, `cycc` places the array in **static storage** but
reserves only **`(N-1) * 8` bytes** for it — one i64 slot short. The next static
object (e.g. a string literal) is laid out at `&a + (N-1)*8`. An in-bounds write
to the array's last element — `store64(&a + (N-1)*8, v)` — therefore overwrites
that neighboring static datum.

`var a[N]` is N i64 slots (8N bytes) throughout the cyrius codebase and stdlib
(e.g. daimon's `argv_buf[4]` written at `&buf + 0/8/16`); the write to slot
`N-1` is in-bounds by the language's own convention. The compiler simply
under-reserves the backing store.

## Minimal reproducer

```cyrius
include "lib/string.cyr"
include "lib/fmt.cyr"
include "lib/alloc.cyr"
include "lib/io.cyr"

fn writes_four_slots(v) {
    var parts[4];
    store64(&parts, v);
    store64(&parts + 8, v);
    store64(&parts + 16, v);
    store64(&parts + 24, v);   # 4th slot of a [4] array — IN BOUNDS
    return &parts;
}

fn main() {
    alloc_init();
    var sp = " ";                       # adjacent static string literal
    # ... print load8(sp), &parts, sp, (&parts - sp), load8(sp) ...
    var p = writes_four_slots(1);
    return 0;
}
```

Observed:

```
space_before=32  parts_addr=5628256  space_lit_addr=5628280  delta=-24  space_after=1
```

- `space_before=32` — the `" "` literal is correct (0x20) before the call.
- `delta = parts_addr - space_lit_addr = -24` — the literal sits exactly at
  `&parts + 24`, i.e. on top of slot 3 (the 4th element) of `parts[4]`.
- `space_after=1` — after `store64(&parts + 24, 1)`, the literal byte is `1`.

So `var parts[4]` got 24 bytes (3 slots) of static backing, not 32.

## Real-world impact in daimon

daimon's `ip_to_cstr` (per-request, in the rate limiter) formatted the peer IP
via `var parts[4]` + `store64(&parts + 24, octet)`. For a `127.0.0.1` client the
4th octet is `1`, which landed on sandhi's single-space `" "` string literal.
`sandhi_server_get_path` then searched the request line for byte `1` instead of
`0x20`, found no space, and returned an empty path — so **every `/v1/*` route
404'd, in both sync and async serve modes.**

The bug is layout-sensitive: it only bites when `parts` happens to be placed
immediately before a live string literal. In daimon it stayed dormant until an
unrelated change (removing three dead functions during the 1.2.6 serve_async
refactor) shifted the static layout so `parts` landed 24 bytes before sandhi's
`" "` literal. That fragility — *deleting dead code silently breaks request
routing* — is the surface symptom; this storage under-reservation is the cause.

## Daimon-side workaround (shipped in 1.2.6)

`ip_to_cstr` was rewritten to compute each octet inline
(`var val = (ip >> (pi * 8)) & 255`) instead of staging them through an
address-taken `var parts[4]`. No `&`-of-local-array, no static placement, bug
avoided. Routing restored; dead-code removal no longer corrupts anything.

This is a workaround, not a fix — any other address-taken `var a[N]` whose last
slot is written remains exposed until cycc reserves the full `N*8` bytes.

## Suggested fix (upstream)

In the pass that promotes address-taken fixed-size local arrays to static
storage, reserve the array's full declared size (`N * 8` bytes for `var a[N]`)
before laying out the next static object. The current reservation appears to be
off by one element (`(N-1) * 8`).
