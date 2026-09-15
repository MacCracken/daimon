# The shipped `daimon-aarch64` binary issues x86_64 syscall numbers

**Status:** 🔴 **OPEN — MEASURED.** `cyrius build --aarch64 src/main.cyr` exits 0 and produces a
valid aarch64 ELF, and that binary cannot bind a socket.
**Filed:** 2026-09-14, by daimon (found during the 2.1.4 toolchain bump).
**Affects:** every `daimon-aarch64` artifact `release.yml` has ever shipped (the block dates from
the port). Not a 6.6.4 regression — the same six warnings print under 6.6.2 (five are daimon's,
the sixth is majra's — see the last section).
**Severity:** **P1.** The x86_64 binary is unaffected; the aarch64 artifact is advertised in
`[release] cross_bins` and is wrong on every network path.

## What was measured

`src/main.cyr:34-42` ("Syscall numbers not in stdlib"), `src/server.cyr:11` and `src/agent.cyr:231`
define syscall numbers as plain globals with x86_64 values. On the aarch64 target the compiler
reports, for each one the stdlib peer also defines:

```
warning:<source>:35:16: duplicate symbol 'SYS_SOCKET' redefined with conflicting value (last definition wins)
warning:<source>:36:17: duplicate symbol 'SYS_CONNECT' redefined with conflicting value (last definition wins)
warning:<source>:38:14: duplicate symbol 'SYS_BIND' redefined with conflicting value (last definition wins)
warning:<source>:39:16: duplicate symbol 'SYS_LISTEN' redefined with conflicting value (last definition wins)
warning:src/server.cyr:11:21: duplicate symbol 'SYS_GETPEERNAME' redefined with conflicting value (last definition wins)
```

daimon's source is parsed AFTER the auto-prepended stdlib, so **daimon's value wins**. Per symbol,
against each peer daimon targets (`-` = the peer does not define it):

| symbol | daimon | x86_64 | aarch64 | agnos | on aarch64 the binary issues |
|---|---:|---:|---:|---:|---|
| `SYS_SOCKET` | 41 | 41 | 198 | - | **wrong; the peer defines 198** — 41 is not a number the aarch64 peer declares |
| `SYS_CONNECT` | 42 | 42 | 203 | - | **wrong; the peer defines 203** |
| `SYS_BIND` | 49 | 49 | 200 | - | **wrong; the peer defines 200** |
| `SYS_LISTEN` | 50 | 50 | 201 | - | **wrong; the peer defines 201** |
| `SYS_GETPEERNAME` | 52 | 52 | 205 | - | **wrong; the peer defines 205.** 52 is `SYS_FCHMOD` in the aarch64 peer's own table — the exact shape bote 3.3.x and the stdlib's `sys_getpeername` note document: it *succeeds* with an untouched buffer |
| `SYS_PIDFD_OPEN` | 434 | 434 | 434 | - | same value — harmless duplicate |
| `SYS_PIDFD_SEND_SIGNAL` | 424 | - | - | - | unified numbering ≥ 424 — correct on both |
| `SYS_ACCEPT` | 43 | - | - | - | aarch64 has no plain `accept` (the peer carries `SYS_ACCEPT4 = 242`); 43 is not a number the peer declares |
| `SYS_GETSOCKOPT` | 55 | - | - | - | neither Linux peer declares `getsockopt`; 55 is not a number the aarch64 peer declares |
| `SYS_RENAME` | 82 | 82 | - | 31 | aarch64 has no `rename` (the peer carries `SYS_RENAMEAT = 38`); 82 is not a number the peer declares |
| `SYS_SETRLIMIT` | 160 | - | - | - | aarch64 has no `setrlimit`; 160 is `SYS_UNAME` in the aarch64 peer's own table |

(Victim names come only from `lib/syscalls_aarch64_linux.cyr`'s own declarations — two of the nine
collide with a name it declares; the other seven are, in the 6.6.4 diagnostic's wording, "not one the
aarch64 stdlib declares (it may still be a DIFFERENT real syscall)". The point is not the specific
victim but that none of them is the intended call.)

⚠ CI cannot see this. The aarch64 lane (`ci.yml` "Cross-build aarch64", `release.yml` likewise)
checks only that the output is an aarch64 ELF; it never runs it, and a `warning:` does not fail the
step. The lane's comment still lists `SYS_FORK` / `SYS_EPOLL_WAIT` / `SYS_OPEN` as known upstream
blockers — all three have been resolved upstream and the build has been green since at least the
6.6.2 pin, so the allowlist is dead code (harmless: it only fires on a failed build).

## Why this is the 6.6.4 class

cyrius 6.6.4's CHANGELOG describes exactly this: *"Raw x86_64 syscall numbers in arch-neutral
stdlib code ran as DIFFERENT syscalls on aarch64-Linux"* — and its closing list names sibling
repos (agnostik, vidya, kavach, …) carrying the same decimal literals for the pin sweep. daimon's
are here. The new `raw_syscall_literals_routed` gate covers `lib/` + `cbt/` only, so a consumer's
own `var SYS_X = <x86 number>` is invisible to it.

## What would close it

1. **Delete the five the stdlib already defines on both Linux peers** — `SYS_SOCKET`,
   `SYS_CONNECT`, `SYS_BIND`, `SYS_LISTEN`, `SYS_GETPEERNAME` (and the harmless `SYS_PIDFD_OPEN`).
   The x86_64 build is byte-for-byte unchanged (same values); the aarch64 build starts using the
   peer's numbers. Zero risk on the host.
2. **Route the rest through `sys_*` wrappers or per-arch spellings**: `SYS_ACCEPT` →
   `sys_accept4(fd, 0, 0, 0)` (the stdlib's own argument at `sys_getpeername`; both peers declare
   `SYS_ACCEPT4`), `SYS_RENAME` → the `renameat(AT_FDCWD, …)` wrapper (both peers declare
   `SYS_RENAMEAT`), and `SYS_GETSOCKOPT` / `SYS_SETRLIMIT` → an `#ifdef CYRIUS_ARCH_AARCH64` arm
   spelling the arm64 kernel numbers (neither Linux peer declares them today — `getsockopt` is 209
   and `prlimit64` 261 on arm64 per the kernel's unistd table; verify against the peer when adding).
   `src/ipc.cyr`'s comment already explains why the `sys_mkdir` there is a wrapper and not a raw
   number; the same reasoning applies to every row above.
3. **Make the lane see it**: fail the aarch64 build step on any `duplicate symbol 'SYS_`
   warning, and drop the three-symbol allowlist that no longer matches anything.
4. Run the aarch64 binary once under `qemu-aarch64` (bind + one HTTP request) before it is
   shipped as a release asset.

## The sixth warning is upstream: majra

```
warning:lib/majra.cyr:205:19: duplicate symbol 'SYS_GETRANDOM' redefined with conflicting value (last definition wins)
```

majra 2.7.2 (the latest tag) declares `var SYS_GETRANDOM = 318;` — x86_64 — in its bundle. The
aarch64 peer defines 278 (agnos 45). Because majra's bundle is prepended AFTER the stdlib leaves,
318 wins on aarch64 for every `getrandom` majra issues. Same class, majra's to fix (the cyrius
6.6.4 CHANGELOG's closing list already names agnostik for the identical `318`). Recorded here so
the next majra bump checks for it; not a daimon edit.

## Related

- `docs/development/issues/2026-09-14-daimon-does-not-build-for-agnos.md` — the agnos target
  hits the same block from the other side (`SYS_RENAME = 31` there; `sys_unlink` arity).
- cyrius CHANGELOG [6.6.4] "Raw x86_64 syscall numbers in arch-neutral stdlib code".
- bote 3.3.x `src/transport_unix.cyr` — the `sys_accept4` reasoning quoted above.
