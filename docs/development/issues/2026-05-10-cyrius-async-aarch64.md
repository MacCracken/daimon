---
name: Cyrius lib/async.cyr SYS_EPOLL_WAIT undefined on aarch64
description: Upstream-stdlib blocker — async.cyr unconditionally references SYS_EPOLL_WAIT (x86_64-only constant)
type: blocker
status: open
filed: 2026-05-10
component: cyrius stdlib (lib/async.cyr)
affects: daimon aarch64 cross-build
---

# Cyrius stdlib blocker — `lib/async.cyr` `SYS_EPOLL_WAIT` undefined on aarch64

**Filed:** 2026-05-10 (during daimon 1.2.0 ship — cyrius 5.10.34).
**Status:** open upstream; daimon CI tolerant via warn-on-detect.
**Severity:** medium — blocks daimon's `--aarch64` cross-build; x86_64 unaffected.

## Symptom

`cyrius build --aarch64 src/main.cyr build/daimon-aarch64` fails with:

```
error:20008: undefined variable 'SYS_EPOLL_WAIT' (missing include or enum?)
compile src/main.cyr -> build/daimon-aarch64 [aarch64] FAIL
```

Line 20008 is inside `lib/async.cyr` after preprocessor expansion. Both
**cyrius 5.10.34** (current daimon pin) and **5.10.47** (latest tag at
filing time) reproduce.

## Root cause

`lib/async.cyr` references `SYS_EPOLL_WAIT` at two call sites:

```
117:    syscall(SYS_EPOLL_WAIT, epfd, &revents, 1, 0 - 1);
145:    var nr = syscall(SYS_EPOLL_WAIT, epfd, &revents, 1, ms);
```

The constant is defined in `lib/syscalls_x86_64_linux.cyr` line 64
(`SYS_EPOLL_WAIT = 232`) but NOT in `lib/syscalls_aarch64_linux.cyr` —
because aarch64 has no plain `epoll_wait` syscall. The aarch64ABI uses
`SYS_EPOLL_PWAIT = 22` instead (with a NULL sigmask + 8-byte sigsetsize).

The async runtime is arch-portable in intent — but the syscall dispatch
is not arch-gated. Fixing requires either:

1. An arch-dispatch shim inside `lib/async.cyr` (`#ifdef CYRIUS_ARCH_AARCH64`
   block calling `SYS_EPOLL_PWAIT` with the 6-arg shape; `#else` calling
   `SYS_EPOLL_WAIT`), OR
2. A new `SYS_EPOLL_WAIT` alias in `lib/syscalls_aarch64_linux.cyr` that
   resolves to a wrapper fn translating to `SYS_EPOLL_PWAIT(…, 0, 8)`.

Option 2 is cleaner (call-site stays portable) and matches how sakshi
2.2.2 handled the `_sk_open` x86=`open` / aarch64=`openat(AT_FDCWD,…)`
arity gap.

## Daimon-side workaround

CI's `aarch64 cross-build (best-effort)` step (in both `.github/workflows/ci.yml`
and `release.yml`) downgrades this specific error to a `::warning::` and
exits 0. Any other aarch64 build failure still fails the step — we want
signal when a daimon-side regression breaks the cross-build, just not
when upstream is the blocker. Pattern is identical to sakshi 2.2.2's
aarch64 lane handling of `vec_get` / `vec_len` stdlib gaps.

The check is a grep on the build output:

```bash
if grep -qE "undefined variable 'SYS_EPOLL_WAIT'" /tmp/aarch64.log; then
  echo "::warning::aarch64 cross-build blocked on upstream stdlib gap..."
  exit 0
fi
```

When upstream fixes the gap, the build succeeds, the warning never fires,
and we get the aarch64 binary back automatically. No daimon-side
follow-up required.

## Tracking

- This file is the canonical daimon-side tracker. Update it when the
  upstream fix lands; remove the warn-on-detect grep + this file when
  the fix is in our pin.
- Upstream fix would land in cyrius's `lib/syscalls_aarch64_linux.cyr`
  or `lib/async.cyr` (cyrius stdlib repo). Not filed there — passive
  tracking until a consumer-driven reason to escalate. Same passive
  posture as agnosys's `2026-05-09-cyrius-ifplat-codegen.md`.

## Daimon doesn't fix this in `src/`

Daimon's async paths (`serve --async`, `async_handle_client`) compile
fine on x86_64 (200 / 200 tests pass; build clean at 622 KB DCE). The
issue is purely an upstream stdlib portability gap surfaced by the
`--aarch64` build mode. A daimon-side workaround (e.g. an `#ifdef`
guard around `serve_async` invocation) would mask the underlying
problem without fixing the stdlib for any other downstream consumer.

## Related

- sakshi 2.2.2 — same posture for `vec_get` / `vec_len` aarch64 stdlib
  emit gaps.
- agnosys `docs/development/issues/2026-05-09-cyrius-ifplat-codegen.md`
  — passive internal tracker for an upstream cyrius regression with no
  daimon-side fix.
