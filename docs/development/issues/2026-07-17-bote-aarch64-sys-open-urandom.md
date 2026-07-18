# bote reads /dev/urandom via `syscall(SYS_OPEN, ...)` — `open(2)` doesn't exist on aarch64 Linux, breaking the daimon aarch64 cross-build

**Status:** OPEN upstream (bote) — filed 2026-07-17 by daimon (consumer),
bote 3.1.4 / daimon 1.4.2. This is a **vendored-stdlib gap** the consumer
*cannot* fix (`lib/bote.cyr` is gitignored, repopulated by `cyrius deps`).

**Severity:** **Low.** Portability-only — no runtime security impact. The
authoritative daimon build target is x86_64 and it is green; aarch64 is a
best-effort bonus lane in CI/release. This gap only blocks producing an
aarch64 binary, and only at link time (undefined symbol), never at runtime.

**Component:** `lib/bote.cyr` — `_gen_session_id()` (line 2951 in the bote
3.1.4 snapshot; `src/session.cyr` in bote's own tree). A second site,
`pkce_code_verifier` (`src/pkce.cyr`), has the same gap. Byte-identical across
bote **3.1.0 / 3.1.1 / 3.1.4**, so this is **pre-existing since daimon 1.4.0**
(first bote-hosting release), not a regression introduced by the 1.4.2
dependency bump.

**Upstream tracking:** filed a bote-authored copy at
`bote/docs/development/issues/2026-07-17-aarch64-sys-open-urandom.md` (in the
sibling bote repo) so it can be repaired at the source.

## Summary

The daimon aarch64 cross-build fails:

```
cyrius build --aarch64 src/main.cyr build/daimon-aarch64
  -> undefined variable 'SYS_OPEN'
```

`SYS_OPEN` is referenced by bote's session-ID generator, which opens
`/dev/urandom` for 16 bytes of entropy:

```cyrius
# lib/bote.cyr : _gen_session_id()
var fd = syscall(SYS_OPEN, "/dev/urandom", 0, 0);   # <- SYS_OPEN undefined on aarch64
...
var r = syscall(SYS_READ, fd, raw + got, 16 - got);
...
syscall(SYS_CLOSE, fd);
```

aarch64 Linux has **no `open(2)` syscall** — the syscall table provides only
`openat(2)` (`SYS_OPENAT`). The aarch64 syscall-number table
(`lib/syscalls_aarch64_linux.cyr`) therefore never defines `SYS_OPEN`, so the
reference is an undefined variable at compile/link time. `SYS_READ`,
`SYS_CLOSE`, and `SYS_EXIT` in the same function are fine — those numbers exist
on both arches; only the `open` entry point is x86_64-only (among the two
arches daimon targets).

## Why the consumer can't fix it

- `lib/bote.cyr` is **vendored** — gitignored, repopulated by `cyrius deps`
  from the pinned bote dist bundle. Any consumer edit is wiped on the next
  resync.
- daimon consumes bote's session-ID path indirectly (bote's builtin MCP tool
  bundle); there is no daimon-side seam to interpose a different `/dev/urandom`
  open on.

## Proposed fix (in bote)

bote's own source has **two** call sites, both bypassing the stdlib's
arch-portable `sys_open` wrapper by calling the raw `SYS_OPEN` constant:

- `src/session.cyr` — `_gen_session_id` (this is the one that surfaces first in
  daimon's vendored-bundle link).
- `src/pkce.cyr` — `pkce_code_verifier` (same gap, immediately behind it).

The stdlib already ships `sys_open(path, flags, mode)`, which resolves to bare
`open(2)` on x86_64 and `openat(AT_FDCWD, ...)` on aarch64. The fix is a
one-line substitution at each site:

```cyrius
var fd = sys_open("/dev/urandom", 0, 0);   # was: syscall(SYS_OPEN, "/dev/urandom", 0, 0)
```

`openat(AT_FDCWD, path, ...)` is semantically identical to `open(path, ...)`
for an absolute path, so `/dev/urandom` opens the same on every arch. Preferred
over an `#if aarch64` fork of the call site: one code path, portable, and the
wrapper already exists. (glibc/musl implement `open()` as
`openat(AT_FDCWD, ...)` on aarch64 for exactly this reason.)

## daimon-side handling (this repo)

Until bote ships the fix, daimon's best-effort aarch64 lane in
`.github/workflows/ci.yml` and `.github/workflows/release.yml` classifies this
as a known upstream gap and downgrades it to a warning (exit 0), keeping the
x86_64 build authoritative. The allowlist regex matching the undefined-symbol
line was extended `SYS_(FORK|EPOLL_WAIT)` → `SYS_(FORK|EPOLL_WAIT|OPEN)` so a
genuine daimon-side regression still fails the step while this upstream blocker
does not.

## References

- **daimon 1.4.0** — first release hosting bote's libro/session tooling; where
  the `SYS_OPEN` reference entered daimon's aarch64 link set.
- Sibling upstream aarch64 syscall gap (cyrius stdlib):
  `2026-05-10-daimon-async-aarch64-sys-epoll-wait.md` (tracked in upstream
  cyrius) — same class (a `SYS_*` the aarch64 table doesn't define), same CI
  handling.
- aarch64 Linux syscall table: no `__NR_open`; `__NR_openat` (56) only. See
  `include/uapi/asm-generic/unistd.h` (the generic table aarch64 uses).
