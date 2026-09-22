# daimon does not build for `--agnos`, and that blocks crab's M7 index

**Status:** ✅ **RESOLVED in daimon 2.1.7 — daimon builds AND boots on AGNOS.**
`cyrius build --agnos src/main.cyr` produces `build/daimon-agnos` (2,948,952 bytes), and that binary
was seeded onto an ext2 rootfs and booted on a **production** agnos kernel (no selftest hook) under
QEMU + gnoboot + OVMF + NVMe, exec'd by kybernet as PID 1 in ring 3:

```
[    6.242024] kybernet: exec /bin/agnsh
[2747164000] [INFO] daimon listening
  daimon v2.1.7 listening on port 8090 (sync)
```

Allocator init, args init, `app_init`, the sakshi logging path and the server banner all run on
agnos. crab's M7 blocker — "a daimon built for agnos cannot be talked to there" — is cleared to the
extent daimon's own surface allows; see §"Scope" below for what that does and does not include.

⭐ **The boot paid for itself twice over.** It found two defects that the 277-test suite could not
see, both of which also affected the HOST build: `daimon_version()` read the `VERSION` file by
RELATIVE path and reported `unknown` from any cwd but the repo root (the boot printed
`daimon vunknown`), and `rate_check`'s `if (ip == 0) { return 1; }` would have failed OPEN on agnos
— the same VULN-009 shape 2.1.6 fixed for aarch64, reached by a completely different cause, because
agnos's `sock_accept` returns a raw conn_id and exposes no getpeername. Both fixed in 2.1.7.

### Scope — what "resolved" does and does not mean

daimon's process-spawn and IPC subsystems are **unreachable from any entry point on every target**:
`agent_spawn_with_limits`, `agent_start`, `agent_ipc_bind`, `ipc_send` and `msg_bus_publish` have
**zero callers**, and the HTTP agent-create handler registers a record via `agent_handle_new` without
spawning a process. The seven `undefined function` warnings in the agnos build (`sys_execve`,
`sys_socket`, `sys_bind`, `sys_listen`, `sys_accept4`, `sys_connect`, `sys_pidfd_open`) therefore sit
in code nothing reaches **on Linux either**, and DCE NOPs them.

⇒ **The agnos build has the same functional surface as the host build.** That is the claim. It is
*not* "agent spawning works on agnos". When spawn/IPC are wired to the API, the agnos mapping
resumes — `sys_spawn_path` (#43) for exec and `chan_op` (#97) for IPC — and the two constraints
measured in the 2.1.6 update still apply (`CH_SEND`'s 64-byte cap vs `MAX_MESSAGE_SIZE` 65536; no
rlimit syscall on agnos, so VULN-010 has no direct equivalent). Tracked in the roadmap, not here.

Archived at 2.1.7. Historical filing follows.

**Filed:** 2026-09-14, by **crab**.
**Affects:** daimon **2.1.3** (and every version before it — no agnos build has ever been attempted).
**Severity:** **P1 — blocking for crab M7/M8, and first-order for daimon.** ⛔ This line used to read
*"latent for daimon itself, which targets the host today"*. That was the wrong assumption and is
retracted: **daimon is the AGNOS agent orchestrator**, every consumer is an AGNOS agent, and a build
that runs only on the host is the exception, not the baseline. The host build is where daimon is
developed; agnos is where it is meant to run.

## ⚠ 2.1.6 update (2026-09-22): 3 errors — and a RETRACTION of how this has been framed

**The "scoping decision" framing is wrong and is retracted here.** This filing (§"What would close
it" step 3) and daimon's roadmap have both carried the idea that agnos support turns on *"a scoping
decision about what daimon IS on agnos"*. That reads as: agnos support is optional and under review.
It is not. **daimon is the AGNOS agent orchestrator** — AGNOS is the primary target and every
consumer is an AGNOS agent. Raised to **P1** in daimon's roadmap.

### crab's step 3 has a concrete answer: it is NOT the real question

Step 3 asks whether `dynlib`, `fdlopen`, `tls`, `mmap` and `net` — all in daimon's `[deps] stdlib`
— are available on agnos, and suggests *"that, not the syscall arity, may be the real question"*.
Measured:

- **`dynlib`, `fdlopen`, `tls` — daimon calls ZERO symbols from all three.** Checked by taking every
  `fn` each module exports and grepping `src/`: 0, 0, 0. They are in `[deps] stdlib` only because
  sandhi's bundle references them at compile time (daimon's `CLAUDE.md` says so explicitly), and
  `CYRIUS_DCE=1` NOPs them. They cannot block an agnos build because nothing reaches them.
- **`mmap` — available.** The agnos peer defines `sys_mmap` and `sys_munmap`.
- **`net` — available.** `lib/net.cyr` already carries **44** `CYRIUS_TARGET_AGNOS` arms.

So step 3 resolves to "no blocker", and the syscall arity *is* the live question after all.

### There is no capability gap

Read from `agnos/kernel/core/syscall.cyr` — the canonical source, 1.57.5, 104 dispatch arms — the
kernel supplies a primitive for every OS-facing thing daimon does:

| daimon subsystem | Linux today | agnos primitive |
|---|---|---|
| HTTP API (8090) | `sys_socket`/`bind`/`listen`/`accept4` | `sys_sock_listen(port)` — bind+listen in one — then `sys_sock_accept`, `sys_sock_recv/send`. |
| agent IPC (`src/ipc.cyr`) | AF_UNIX socket at a filesystem path | **`chan_op` #97 capability channels.** `CH_MINT` returns an unnamed **pair**; `CH_ENDOW` arms one end for placement into the next `spawn_path` child, so the child inherits the endpoint at spawn. |
| agent spawn (`src/agent.cyr`) | `fork` + `execve` + `prlimit64` | `sys_spawn_path` #43 — one call, and it **does** tokenize argv from the command line. |
| supervisor `/proc` reads | `/proc/{pid}/status`, `/stat`, `/fd`, `/task` | agnos has **no `/proc`**. `sys_proclist` #99 returns 64-byte records: pid · state · ppid · name[32] · `+56` split as **cpu ticks (low u32) / rss pages (high u32)** — the same two numbers `read_vm_rss` / `read_cpu_time_ms` parse today. |

⭐ **The IPC mapping is an UPGRADE, not a workaround.** An AF_UNIX server binds a *name* on the
filesystem, which is why `agent_ipc_bind` must `unlink` the path first — and that unlink-then-bind
window is a race. An agnos channel has **no name**: it is minted as a pair and one end is handed to
the child by the spawn itself. The kernel says so directly (`kernel/core/syscall.cyr:9140`) — the
pair design *"deletes the entire unlink-before-bind race class AF_UNIX carries."*

### Two real constraints, stated rather than hand-waved

1. **`CH_SEND` caps a payload at 64 bytes** (`CH_SEND = 0x02; (fd, buf, len<=64)`) while daimon's
   `MAX_MESSAGE_SIZE` is **65536** (`src/ipc.cyr:17`). Bulk bodies need `sys_shm_create` /
   `sys_shm_write` / `sys_shm_read` / `sys_shm_free`, with the channel carrying the control word and
   the shm id. A design decision to take, not a blocker.
2. **agnos has no rlimit syscall at all** — `grep -ci 'rlimit'` over `kernel/core/syscall.cyr`
   returns **0**. VULN-010's per-agent `RLIMIT_AS` / `RLIMIT_CPU` has no direct equivalent. Whether
   that becomes a kernel ask, a scheduler-side quota, or a documented non-guarantee on agnos is a
   question **for the agnos side**, and should be asked rather than assumed in either direction.

### What the 3 errors are now

The 2.1.6 raw-syscall sweep resolved 2.1.5's undefined `SYS_EXECVE` / `SYS_WAIT4`. What surfaces is
that the agnos peer spells the same wrappers differently, because agnos carries an **explicit-length
invariant** — every path argument carries its length, with no NUL-termination assumption:

```
error:src/agent.cyr:344: 'sys_waitpid' expects 1 argument, got 3     # agnos: sys_waitpid(pid)
error:src/agent.cyr:347: 'sys_waitpid' expects 1 argument, got 3
error:src/memory.cyr:89: 'sys_rename' expects 4 arguments, got 2     # agnos: sys_rename(old, oldlen, new, newlen)
```

**Strictly better than 2.1.5.** `src/memory.cyr` previously *compiled* on agnos against daimon's
`SYS_RENAME = 82`, while agnos's `rename` is **31** — the atomic-write path issued a silently wrong
syscall. It is a loud compile error now. Same win as the aarch64 half of 2.1.6.

⚠ **Do not treat `lib/syscalls_x86_64_agnos.cyr` as authority.** Its own header records that a
doc→peer→doc citation loop once let a wrong number verify itself, and states the kernel is canonical.
It mirrors 1.56.x against a 1.57.5 kernel. Every claim above was read from the kernel. The first
draft of this note was written from the peer and asserted agnos was "TCP/IP only" with no local IPC
— wrong, and exactly the failure mode the peer's own header warns about.

## ✅ 2.1.5 update (2026-09-22): 53 errors → 3 — the bote/cyrius half is CLOSED

daimon 2.1.5 moved the pin to **cyrius 6.6.6** and bote to **3.3.13**. `cyrius build --agnos
src/main.cyr` now reports **three errors, and `syscalls_linux_common` does not appear in the agnos
translation unit at all** (`grep -c syscalls_linux_common` over the build log: 0). The 50 errors
attributed to the Linux peer compiling beside the standalone agnos peer are gone; what is left is
precisely the "3 daimon sites" this filing always named:

```
error:src/agent.cyr:262:27: undefined variable 'SYS_EXECVE' (missing include or enum?)
error:src/agent.cyr:321:35: undefined variable 'SYS_WAIT4'  (missing include or enum?)
error:src/agent.cyr:324:30: undefined variable 'SYS_WAIT4'  (missing include or enum?)
```

cyrius 6.6.6 also made `lib/io.cyr` self-sufficient (it includes `lib/args_agnos.cyr` itself), which
closed the `_agnos_getenv` gap that sibling consumers hit on the same target.

**What remains is a daimon change, not a wait.** An agnos spawn arm for `execve` / `wait4` in
`src/agent.cyr`. Deliberately not done in 2.1.5: that release is pin-only, and an agnos spawn path
needs its own tests. ⛔ This paragraph originally continued *"plus the scoping decision this filing
already raises on whether `dynlib` / `fdlopen` / `tls` / `mmap` / `net` mean anything on agnos"* —
**retracted at 2.1.6**, which measured it: daimon calls **zero** symbols from `dynlib`, `fdlopen`
and `tls`, and `mmap` and `net` both have agnos support. See the 2.1.6 update.

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

   > ✅ **ANSWERED at daimon 2.1.6 — this is NOT the real question.** daimon calls **zero** symbols
   > from `dynlib`, `fdlopen` and `tls` (measured: every `fn` each module exports, grepped against
   > `src/` — 0/0/0); they are in `[deps] stdlib` only so sandhi's bundle resolves at compile time,
   > and `CYRIUS_DCE=1` NOPs them. `mmap` has `sys_mmap`/`sys_munmap` on agnos, and `lib/net.cyr`
   > carries 44 agnos arms. The syscall/wrapper arity **is** the live question. crab was right to
   > flag the uncertainty and right not to assume the answer; the answer is "no blocker".

⚠ **crab will not work around this.** Faking an index crab cannot back, or shipping surfaces that
look like the roadmap's mock and under-deliver, is the failure crab's own VOLUMES entry exists to
name: *"crab could have shipped a probe. It deliberately did not."*

## Related

- crab `cyrius.cyml` `[deps.daimon]` — the declaration, with the "declared, not linked" reasoning.
- crab `docs/development/roadmap.md` § M7 — what the index is for and what it must not require.
