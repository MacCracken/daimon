# The `daimon-aarch64` binary silently loses its per-IP rate limiter and agent rlimits — two x86_64 syscall numbers run as different syscalls there

**Status:** ✅ **RESOLVED in daimon 2.1.6** (see the 2.1.6 update below). Historical status: 🔴 **OPEN — MEASURED.** `cyrius build --aarch64 src/main.cyr` exits 0, prints five daimon
`duplicate symbol 'SYS_…' redefined with conflicting value` warnings (plus one from majra's bundle),
and produces a valid aarch64 ELF that starts, binds and serves. Seven of daimon's nine hand-spelled
x86_64 syscall numbers reach the kernel as the correct native calls anyway; **two do not**, and each
of those turns off a hardening layer without an error.
**Filed:** 2026-09-14, by daimon (found during the 2.1.4 toolchain bump; the first cut of this filing
claimed the binary "cannot bind a socket" — that was wrong, see §"Why seven of nine are fine", and was
corrected the same day after adversarial verification against the cyrius emitter).
**Affects:** every `daimon-aarch64` artifact `release.yml` has ever shipped (the block dates from the
port). Not a 6.6.4 regression — the same warnings print under 6.6.2, and the routing table is the
same for every number involved.
**Severity:** **P2 — two silent hardening regressions, not an outage.** The x86_64 binary is unaffected.

## ✅ 2.1.6 update (2026-09-22): RESOLVED — both defects fixed and verified under qemu

daimon 2.1.6 deleted **all 11** `var SYS_* = <x86_64 number>` globals and swept **40 raw
`syscall(SYS_*, …)` sites to 0** (`sys_*` wrapper calls go 5 → 43). Four raw syscalls remain and
every one is namespaced `_DAIMON_SYS_*` in the new `src/syscalls.cyr` — a prefix that **cannot
shadow a peer symbol**, which is the root cause rather than the eleven symptoms.

- **`SYS_GETPEERNAME` 52 → `sys_getpeername`**, which resolves the peer's 205 on aarch64.
- **`SYS_SETRLIMIT` 160 → `prlimit64`** (302 x86_64 / 261 aarch64, from the UAPI headers), **with
  the return checked** — on failure the child exits 126 rather than exec'ing unconfined, distinct
  from the 127 used for a failed exec. Not an `#ifdef` around `setrlimit`: the generic ABI
  supersedes get/setrlimit with `prlimit64` outright, so one code path serves both arches.
- `SYS_ACCEPT` 43 → `sys_accept4(fd, 0, 0, 0)`; `SYS_RENAME` 82 → `sys_rename` (aarch64 arm routes
  to `renameat`); `SYS_WAIT4` → `sys_waitpid`.

**aarch64 `duplicate symbol 'SYS_…'` warnings: 5 → 0.**

**Verified by running the binary, which is what this filing asked for.** Under `qemu-aarch64` the
cross-built `daimon-aarch64` serves `/v1/health`, and over 130 requests from one IP returns
**119 × 200 then 11 × 429** — the limiter engaging at exactly `RATE_LIMIT_MAX = 120`. Before the
fix `get_peer_ip` returned 0 for every caller and `rate_check`'s `if (ip == 0) { return 1; }`
allowed all 130. `tests/syscall_portability.tcyr` (17 assertions, arch-relative by construction)
passes **17/17 on x86_64 and on aarch64 under qemu**; against a mutant restoring the 2.1.5
shadowing, **9 of 17 fail** on aarch64.

**The lane now sees it**: CI fails the aarch64 step on any `duplicate symbol 'SYS_` warning (the
build exits 0 either way, so only that check can catch it), and a best-effort step runs the
portability test under `qemu-aarch64`. Fix items 1–3 of §"Fix" are done; the `/proc/self/limits`
half of item 3 is covered by `syscall_portability`'s prlimit64 read/write/restore assertions
instead of a spawned child.

## ⚠ 2.1.5 update (2026-09-22): re-verified under cyrius 6.6.6 — daimon's half UNCHANGED, majra's half CLOSED

The `ESYSXLAT` routed set **grew 44 → 60 rows** at 6.6.6. Re-derived here by disassembling the shipped
artifact rather than reading it off a changelog (`aarch64-linux-gnu-objdump -d build/daimon-aarch64`,
walking the `cmp x8,#SRC` / `mov x8,#DST` chain): 60 rows, source numbers

```
0 1 2 3 4 5 6 7 9 10 11 12 16 22 33 35 39 41 42 43 44 45 46 47 48 49 50 51 54 55 60 72 73 74 75 76
77 79 82 83 84 87 88 89 137 138 217 228 232 262 263 269 280 319 1022 1039 1049 1054 1073 1074
```

**Neither 52 nor 160 is among the sixteen added**, so both defects in the table below stand exactly as
filed; the seven correct ones are still correct (41→198, 42→203, 43→202, 49→200, 50→201, 55→209,
82→38, and 434/424 pass through under unified numbering). Status stays 🔴 OPEN.

**The sixth warning — majra's — is RESOLVED.** majra 2.9.1 (daimon 2.1.5 moved the pin from 2.7.2)
replaced the raw `syscall(318, …)` in `uuid_generate` with the per-target `sys_getrandom` wrapper. The
`duplicate symbol 'SYS_GETRANDOM'` line is gone from daimon's aarch64 build, which now emits exactly
the five daimon-owned warnings quoted below and no sixth. See §"The sixth warning is upstream: majra".

## What was measured

`src/main.cyr:34-42` ("Syscall numbers not in stdlib"), `src/server.cyr:11` and `src/agent.cyr:231`
define syscall numbers as plain globals with x86_64 values. daimon's source is parsed AFTER the
auto-prepended stdlib, so where the aarch64 peer declares the same name **daimon's value wins**, and
the compiler says so:

```
warning:<source>:35:16: duplicate symbol 'SYS_SOCKET' redefined with conflicting value (last definition wins)
warning:<source>:36:17: duplicate symbol 'SYS_CONNECT' redefined with conflicting value (last definition wins)
warning:<source>:38:14: duplicate symbol 'SYS_BIND' redefined with conflicting value (last definition wins)
warning:<source>:39:16: duplicate symbol 'SYS_LISTEN' redefined with conflicting value (last definition wins)
warning:src/server.cyr:11:21: duplicate symbol 'SYS_GETPEERNAME' redefined with conflicting value (last definition wins)
```

### Why seven of nine are fine: ESYSXLAT

cyrius's aarch64 backend emits `ESYSXLAT` (`src/backend/aarch64/emit.cyr`), a **runtime** renumbering
chain — `cmp x8,#N` rows on the syscall-number register, so it applies to a `var SYS_X = N` global
exactly as to a literal — that rewrites 38 x86_64 numbers to their aarch64 equivalents and passes every
other number through to `svc` verbatim. The routed set, decoded with the awk pipeline
`tests/gates/platform/raw_syscall_literals_routed.sh` uses, is identical at 6.6.2 and 6.6.4 for every
number below:

| symbol | daimon | x86_64 peer | aarch64 peer | ESYSXLAT | what the aarch64 binary actually issues |
|---|---:|---:|---:|---|---|
| `SYS_SOCKET` | 41 | 41 | 198 | routed 41 → 198 | `socket` ✓ |
| `SYS_CONNECT` | 42 | 42 | 203 | routed 42 → 203 | `connect` ✓ |
| `SYS_ACCEPT` | 43 | - | - | routed 43 → 202 | `accept` ✓ |
| `SYS_BIND` | 49 | 49 | 200 | routed 49 → 200 | `bind` ✓ |
| `SYS_LISTEN` | 50 | 50 | 201 | routed 50 → 201 | `listen` ✓ |
| `SYS_GETSOCKOPT` | 55 | - | - | routed 55 → 209 | `getsockopt` ✓ |
| `SYS_RENAME` | 82 | 82 | - | routed 82 → `renameat` 38 (+ `AT_FDCWD` arg-shift) | `renameat` ✓ |
| `SYS_PIDFD_OPEN` | 434 | 434 | 434 | unified numbering | ✓ (harmless duplicate, same value) |
| `SYS_PIDFD_SEND_SIGNAL` | 424 | - | - | unified numbering | ✓ |
| **`SYS_GETPEERNAME`** | **52** | 52 | 205 | **not routed** | **`fchmod`** — `SYS_FCHMOD = 52` in the aarch64 peer's own table (`lib/syscalls_aarch64_linux.cyr:216`); the peer's `:232-234` explains the row is deliberately absent because an entry would remap real `fchmod` calls |
| **`SYS_SETRLIMIT`** | **160** | - | - | **not routed** | **`uname`** — `SYS_UNAME = 160` (`lib/syscalls_aarch64_linux.cyr:187`) |

(The HTTP listener does not even use daimon's globals: sandhi goes through `lib/net.cyr`'s
`sock_bind` / `sock_listen`, which carry their own x86-numbered `NSYS_BIND = 49` / `NSYS_LISTEN = 50`
and lean on the same rows.)

### The two that are wrong, and what each one does

**`SYS_GETPEERNAME` 52 → `fchmod` — the VULN-009 per-IP rate limiter is disabled for every request.**
The only live site is `src/server.cyr:25` in `get_peer_ip`, called from `rate_check` (`:64`), which is
the first thing `handle_request` does (`:104`). On aarch64 the call is `fchmod(cfd, mode = <heap
pointer>)` on the connected socket's sockfs inode — and, as the stdlib's own note at
`lib/syscalls_linux_common.cyr:505-509` records (*"on aarch64, 52 is fchmod, and it SUCCEEDS — the
caller believes it obtained a peer address while its buffer is untouched garbage. Verified on real
pi."*), it returns 0. So the `r < 0` guard at `:26` does not fire, `load32(addr_buf + 4)` reads a
never-written bump-allocator buffer (zero), `get_peer_ip` returns 0, and `rate_check` takes its
`if (ip == 0) { return 1; }  # Can't determine IP, allow` path at `:65`. Every client is unlimited.
daimon's 52 also overrides the peer's 205 for the stdlib's `sys_getpeername` and sandhi's
`sandhi_server_peer_sockaddr`, but nothing in daimon reaches either.

**`SYS_SETRLIMIT` 160 → `uname` — the VULN-010 agent memory/CPU limits are never applied.** Both sites
are in the forked child (`src/agent.cyr:248` `RLIMIT_AS`, `:254` `RLIMIT_CPU`), both discard the
return value, and execution falls through to the exec. On aarch64 they run as `uname(buf = 9)` and
`uname(buf = 0)` — an unmapped pointer, so the kernel returns `-EFAULT` and writes nothing anywhere.
Unchecked, so the child execs with no address-space or CPU limit. ⚠ This one produces **no build
warning**: neither Linux peer declares `SYS_SETRLIMIT` (or `SYS_PRLIMIT64`), so there is no
conflicting value for the compiler to report. arm64's kernel numbers are `setrlimit` 164 (under
`__ARCH_WANT_SET_GET_RLIMIT`) and `prlimit64` 261 (unconditional); the earlier claim here that
"aarch64 has no setrlimit" was unsupported.

⚠ CI cannot see either. The aarch64 lane (`ci.yml` "Cross-build aarch64", `release.yml` likewise)
checks only that the output is an aarch64 ELF; it never runs it, and a `warning:` does not fail the
step. The lane's comment still lists `SYS_FORK` / `SYS_EPOLL_WAIT` / `SYS_OPEN` as known upstream
blockers — all three have been resolved upstream and the build has been green since at least the
6.6.2 pin, so the allowlist is dead code (harmless: it only fires on a failed build).

## Why this is the 6.6.4 class

cyrius 6.6.4's CHANGELOG describes exactly this: *"Raw x86_64 syscall numbers in arch-neutral stdlib
code ran as DIFFERENT syscalls on aarch64-Linux"* — and its closing list names sibling repos for the
consumer pin sweep. daimon's are here. The new `raw_syscall_literals_routed` gate covers cyrius's
`lib/` + `cbt/` only, so a consumer's own `var SYS_X = <x86 number>` is invisible to it; and the
v6.5.51 raw-literal diagnostic fires only for a *literal* with a row in the generated xlat table, which
neither of daimon's two real defects is.

## What would close it

1. **Delete the five globals the stdlib declares on both Linux peers** — `SYS_SOCKET`, `SYS_CONNECT`,
   `SYS_BIND`, `SYS_LISTEN`, `SYS_GETPEERNAME` — plus the harmless `SYS_PIDFD_OPEN`. The x86_64 build
   is byte-for-byte unchanged (same values); on aarch64 the peer's 205 takes over for `getpeername`,
   which **is** the rate-limiter fix. Zero risk on the host.
2. **`SYS_SETRLIMIT` → an `#ifdef CYRIUS_ARCH_AARCH64` arm spelling `prlimit64` 261 (pid 0, resource,
   new, NULL)** — or, better, a stdlib `sys_setrlimit` wrapper if one lands — and **check the return**
   in the child, since a limit that silently fails to apply is what this filing is about. `SYS_ACCEPT`,
   `SYS_GETSOCKOPT`, `SYS_RENAME` and the two pidfd numbers are routed / unified and need no arm; spell
   them via the peer (`SYS_ACCEPT4`, `SYS_RENAMEAT`, …) only if a wrapper is being written anyway.
3. **Make the lane see it**: fail the aarch64 build step on any `duplicate symbol 'SYS_` warning (a
   cheap belt — it catches 52 but, per above, **not** 160), drop the three-symbol allowlist that no
   longer matches anything, and run the binary once under `qemu-aarch64` asserting (i) a 429 after
   `RATE_LIMIT_MAX` requests from one IP and (ii) a spawned child's `/proc/self/limits` shows the
   RLIMIT_AS / RLIMIT_CPU values. Those two assertions are what actually prove the two defects closed.

## The sixth warning is upstream: majra

```
warning:lib/majra.cyr:205:19: duplicate symbol 'SYS_GETRANDOM' redefined with conflicting value (last definition wins)
```

✅ **RESOLVED at daimon 2.1.5 (majra 2.9.1).** Historical description follows.

majra 2.7.2 declared `var SYS_GETRANDOM = 318;` — x86_64 — in its bundle; the aarch64
peer defines 278 (agnos 45), 318 is not an ESYSXLAT row, and the bundle is prepended after the stdlib
leaves, so 318 wins on aarch64 for every `getrandom` in the translation unit. Filed with majra as
`docs/development/issues/2026-09-14-raw-x86-syscall-numbers-aarch64.md` together with two more
unrouted sites the sweep found there (`_SYS_FCHMOD` 91 on its IPC access-control path, raw
`syscall(35)` in its DAG backoff). Not a daimon edit; recorded so the next majra bump checks for it. **The next bump was daimon 2.1.5 → majra 2.9.1, and it did: the warning is gone.**

## Related

- majra `docs/development/issues/2026-09-14-raw-x86-syscall-numbers-aarch64.md` — the upstream filing.
- `docs/development/issues/2026-09-14-daimon-does-not-build-for-agnos.md` — the agnos target hits the
  same block from the other side (`SYS_RENAME = 31` there; `sys_unlink` arity).
- cyrius CHANGELOG [6.6.4] "Raw x86_64 syscall numbers in arch-neutral stdlib code";
  `tests/gates/platform/raw_syscall_literals_routed.sh` — the derivation of the routed set.
- `lib/syscalls_linux_common.cyr:505-517` — the stdlib's `sys_getpeername` note that measured the
  52-is-fchmod-and-succeeds shape on real hardware.
