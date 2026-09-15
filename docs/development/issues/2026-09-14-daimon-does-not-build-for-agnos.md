# daimon does not build for `--agnos`, and that blocks crab's M7 index

**Status:** 🔴 **OPEN — MEASURED, not read.** `cyrius build --agnos src/main.cyr` fails at daimon
2.1.3 with **53 errors across 36 distinct undefined symbols**.
**Filed:** 2026-09-14, by **crab**.
**Affects:** daimon **2.1.3** (and every version before it — no agnos build has ever been attempted).
**Severity:** **Blocking for crab M7/M8.** Latent for daimon itself, which targets the host today.

## ⚠ 2.1.4 update (2026-09-14): re-measured under cyrius 6.6.4 — hypothesis §"LIKELY ROOT" REFUTED

daimon 2.1.4 moved the pin to **6.6.4** and re-vendored every dep (`cyrius deps` from a lock
restored to HEAD). `cyrius build --agnos src/main.cyr` then reports the **same 53 errors, the same
36 symbols, the same 50/3 split** between `lib/syscalls_linux_common.cyr` and `src/agent.cyr`. The
vendored-snapshot drift was real (the `lib/ai-hwaccel.cyr` lock hash, below) but it was not the
cause of this.

**The measured root is upstream, in two halves:**

1. **bote's `dist/bote.deps` sidecar names `syscalls_linux_common` as a stdlib leaf** (3.3.8 and
   3.3.9 alike). bote's own `[deps] stdlib` does not declare it and its sources mention it only in a
   comment — the sidecar entry comes from `cyrius distlib`'s leaf resolution, which maps each symbol
   the bundle calls (`sys_accept4`, `sys_getpeername`, …) to the stdlib file that *defines* it.
   On the producer's host that file is `lib/syscalls_linux_common.cyr`, the Linux-internal peer
   that `lib/syscalls.cyr` reaches only through `#ifdef CYRIUS_TARGET_LINUX` → the per-arch
   Linux peer's `include`. distlib recorded the peer, not the `syscalls` dispatch umbrella.
2. **`cyrius deps` prepends every sidecar leaf as a target-blind top-level `include`** (cbt
   `deps.cyr` `_dep_pull_leaves` → cbt `build.cyr` `_dep_includes` prepend). So on `--agnos`,
   `lib/syscalls_linux_common.cyr` is compiled next to the STANDALONE `lib/syscalls_x86_64_agnos.cyr`
   that the dispatch selected — which is exactly the "should not be compiling for agnos at all" shape
   observed above. Every one of the 50 is that file's Linux-only `SYS_*` / `EPOLL_CLOEXEC` /
   `LINUX_REBOOT_MAGIC*` names, plus 6 arity collisions against the agnos peer's same-named wrappers.

Nothing in daimon's `[deps] stdlib` or `src/` names `syscalls_linux_common`; no daimon-side edit
removes it from the include set. **Where it closes:** distlib should not name a per-target internal
peer as a sidecar leaf (record the umbrella a consumer would include), or the consumer resolver
should not prepend a sidecar leaf the dispatch umbrella already owns. Either is a cyrius (`cbt`)
change; regenerating `dist/bote.deps` afterwards clears bote. The 3 in `src/agent.cyr` remain
daimon's, as §"What would close it" item 2 says.

**Also re-measured:** the `cyrius.lock` rewrite this filing observed is the class cyrius 6.6.4 now
refuses (its lock carries a `cyrius	<pin>` trailer and a moved snapshot hash under an unchanged pin
aborts the build). At 2.1.4 the lock is re-locked under 6.6.4 and verifies.

## Why crab is filing it

crab **declared daimon** on 2026-09-14 (`[deps.daimon]`, pinned 2.1.3) — the operator's ruling, and
the oldest open item on crab's roadmap. That declaration unblocked crab's M7 (`the index`: a local
index, tags, smart folders) and M8 (assisted search), both of which name daimon as the back end.

⛔ **crab runs ON agnos.** A daimon that cannot be BUILT for agnos cannot be talked to there, so the
index and tags halves of M7 are blocked here rather than in crab. crab's declaration is deliberately
**not a link** — daimon is a binary with no `dist/`, and crab talks to the AF_UNIX socket it binds
per agent (`agent_ipc_new`) over agnos's `sock_connect` #47 / `sock_listen` #56 / `sock_accept` #57 —
so nothing about this is crab folding daimon's code in. It needs daimon to RUN.

⚠ **crab has shipped the half that does not need daimon**, so this is not holding a release: duplicate
detection (crab 0.10.0, `Shift+D`) groups by content hash with a free size pre-filter and works with
no daimon on the box. That is the standing rule crab's declaration commits to — *the index is an
enrichment, not a precondition.* What is blocked is everything that needs a **persisted, disk-wide**
index: tags, `Untagged`, `Unrated`, and the `Local · N files` surface.

## What was measured

```
$ cyrius build --agnos src/main.cyr /tmp/daimon_agnos
53 errors, 36 distinct undefined symbols
  50 in lib/syscalls_linux_common.cyr     <- a VENDORED stdlib file, compiled for the wrong target
   3 in src/agent.cyr                     <- daimon's own source
```

**The 50:** `SYS_OPENAT`, `SYS_MKDIRAT`, `SYS_NEWFSTATAT`, `SYS_UNLINKAT`, `SYS_LINKAT`,
`SYS_RENAMEAT`, `SYS_FCHMODAT`, `SYS_FSTAT`, `SYS_IOCTL`, `SYS_EXECVE`, `SYS_WAIT4`, `EPOLL_CLOEXEC`,
`SYS_GETEUID`/`GETEGID`/`GETGID`/`GETPPID`, `SYS_CHDIR`, `SYS_EXIT_GROUP` … — Linux syscall numbers
agnos does not define.

⛔⛆ **AND THAT FILE SHOULD NOT BE COMPILING FOR AGNOS AT ALL.** `lib/syscalls_x86_64_agnos.cyr` says
so in its own header: *"STANDALONE — does NOT include syscalls_linux_common.cyr"*. Something is
pulling the Linux peer in on the agnos target.

⚠ **A LIKELY ROOT, NOT YET CONFIRMED — the vendored stdlib is a toolchain version behind.** daimon
pins `cyrius = "6.6.2"`; crab pins `6.6.4`, and the two repos' vendored `lib/syscalls_x86_64_agnos.cyr`
**differ** (md5 `928cf01e…` vs `eef0518b…`). Re-running `cyrius deps` against 6.6.4 is the cheapest
thing to try first and may account for most of the 50.

⚠ **AND A SECOND PIECE OF EVIDENCE ABOUT THE VENDORED STATE, found by accident:** running
`cyrius build` rewrote `cyrius.lock` — the recorded hash for `lib/ai-hwaccel.cyr` did not match the
file on disk (`9db30177…` in the lock, `88901cb4…` actual), while `git status lib/` was CLEAN. So the
LOCK was stale, not the file. That is the same class of drift as the pin gap above and points the
same way. ⚠ The lock was restored (`git checkout -- cyrius.lock`); this repo is left as it was found
apart from the prepared fix and this filing.

**The 3 in daimon's own source** are `src/agent.cyr:262` (`SYS_EXECVE`) and `:321`/`:324` (`SYS_WAIT4`)
— process spawning, which agnos exposes differently. These are daimon's to decide: an `#ifdef` arm,
or an agnos-native spawn.

## ⭐ One part is already prepared, in this repo

The FIRST failure class was four `sys_unlink` arity errors — agnos's takes `(path, pathlen)`, the
host's takes a NUL-terminated path alone:

```
error: 'sys_unlink' expects 2 arguments, got 1
  src/memory.cyr:113, src/memory.cyr:149, src/ipc.cyr:183, src/ipc.cyr:347
```

⇒ **Fixed here**: `daimon_unlink(path_cstr, path_len)` in `src/error.cyr` (first in the include order,
before both callers), with the two-target `#ifdef` arm, and all four call sites converted. The shape
is crab's `crab_fs_unlink_raw` (crab `src/app.cyr:1306`) — the same problem solved once already.
⚠ **Uncommitted and unversioned**: daimon's own rule is *"NEVER bump VERSION without the user's
express permission"*, and commits are the operator's. The change is prepared, not cut.

## What would close it

1. Re-run `cyrius deps` against a current toolchain pin and re-measure — the vendored-snapshot drift
   is the cheapest hypothesis and may clear most of the 50.
2. Decide the agnos arm for `execve`/`wait4` in `src/agent.cyr`.
3. Then re-measure: `dynlib`, `fdlopen`, `tls`, `mmap` and `net` are all in daimon's `[deps] stdlib`
   and none is obviously available on agnos. **That, not the syscall arity, may be the real question**
   — and it is a scoping decision about what daimon IS on agnos, which is daimon's to make, not
   crab's to assume.

⚠ **crab will not work around this.** Faking an index crab cannot back, or shipping surfaces that
look like the roadmap's mock and under-deliver, is the failure crab's own VOLUMES entry exists to
name: *"crab could have shipped a probe. It deliberately did not."*

## Related

- crab `cyrius.cyml` `[deps.daimon]` — the declaration, with the "declared, not linked" reasoning.
- crab `docs/development/roadmap.md` § M7 — what the index is for and what it must not require.
