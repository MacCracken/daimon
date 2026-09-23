# Daimon — Claude Code Instructions

## Project Identity

**Daimon** (Greek: δαίμων — guiding spirit) — AGNOS agent orchestrator

- **Type**: Service binary + library
- **Language**: Cyrius (ported from 9,724 LOC Rust)
- **Purpose**: AGNOS agent orchestrator — HTTP API, supervisor, IPC, scheduler, federation, edge fleet, memory, MCP dispatch (binds `127.0.0.1:8090` by default)
- **License**: GPL-3.0-only
- **Cyrius**: 6.6.6 (pinned in `cyrius.cyml`, which is authoritative for every pin below)
- **Version**: `MAJOR.MINOR.PATCH` in the `VERSION` file (not CalVer). `./scripts/version-bump.sh <v>` writes `VERSION` and `src/config.cyr`'s `DAIMON_VERSION` together; `tests/version_sync.tcyr` fails if they drift
- **Genesis repo**: [agnosticos](https://github.com/MacCracken/agnosticos)
- **Philosophy**: [AGNOS Philosophy & Intention](https://github.com/MacCracken/agnosticos/blob/main/docs/philosophy.md)
- **Standards**: [First-Party Standards](https://github.com/MacCracken/agnosticos/blob/main/docs/development/applications/first-party-standards.md)
- **Recipes**: [zugot](https://github.com/MacCracken/zugot) — takumi build recipes
- **Language ref**: [cyrius](https://github.com/MacCracken/cyrius) — compiler, stdlib, docs
- **Port reference**: [vidya](https://github.com/MacCracken/vidya) — first completed Rust→Cyrius port

## Cyrius Stdlib — Available Modules

The following stdlib modules are available via `cyrius.cyml` deps. **Async IS available.**

| Module | Purpose |
|--------|---------|
| `async` | **Cooperative async runtime — epoll event loop, spawn, sleep, await_readable, timeout** |
| `thread` | Clone-based threads, mutex, MPSC channels. **daimon starts no thread** (2.3.4, ADR-006). 2.3.3 ran the agent channels on one, and from the moment it started every `alloc()` took the stdlib's heap lock (`alloc(64)` 10 → 53 ns); `tests/ipc.tcyr` checks `_threads_active` stays 0. Only `tests/agent.tcyr` includes it (its thread-count tests start one). |
| `net` | TCP sockets (connect, listen, accept, read, write) |
| `sandhi` | **In use as of 1.1.4** (`lib/sandhi.cyr` is sandhi 1.9.17) — drives both serve modes in `src/server.cyr`: sync `sandhi_server_run_opts`, async `sandhi_server_run_async`, each bound to `server_bind_addr()` (config `listen_addr`, 127.0.0.1 unless `serve --listen`). The `http_*` shims in `src/http.cyr` are sandhi-backed. Daimon does NOT use sandhi's HTTP/2 / SSE / RPC modules at runtime — but their compile-time deps (`tls`, `mmap`, `dynlib`, `fdlopen`) ARE in `[deps].stdlib`, and `sigil` is an explicit `[deps.sigil]` git pin (see the `sigil` row below), because sandhi's bundle unconditionally references `TLS_EARLY_DATA_ACCEPTED` (0-RTT client-write path) and, since 6.3.43, `sha384_init_into` (defined in `sigil`, called by `tls_native_lowlevel`). DCE NOPs the unused runtime; `sandhi_rpc_mcp_call` is the 1.2.1 hook for unstubbing external MCP forwarding (see `api_mcp_call`). |
| `tls`, `mmap`, `dynlib`, `fdlopen` | Pulled in transitively by sandhi's bundle. Daimon does not call any of these directly today — present for compile-time symbol resolution only. (`tls_native` is split into per-concern peers — `tls_native_conn/ctx/hs12/hs13/keysched/lowlevel` — all resolved transitively.) |
| `sigil` (3.12.18) | Crypto (sha256/sha384/hmac/ed25519/ML-DSA…). Provides the `sha384_init_into` that `tls_native_lowlevel` calls (0-RTT / handshake path) plus the symbols libro/bote consume. **Moved from `[deps].stdlib` to an explicit `[deps.sigil]` git pin at 1.4.2**: sigil 3.12.x ships a *modular* dist (`dist/sigil-mldsa.cyr`, `dist/sigil-sha.cyr`, …) which libro 2.8.x's `.deps` sidecar pulls, and that collided with the monolithic `dist/sigil.cyr` the stdlib snapshot vendored — 227 `duplicate fn (last definition wins)` warnings between two packagings of the *same* release. The explicit top-level pin at `modules=["dist/sigil.cyr"]` (self-contained umbrella) overrides the transitive modular selection so exactly one packaging is vendored. Mirrors bote/libro/majra, which all pin sigil explicitly. |
| `bayan` (1.5.6) | JSON parse/emit (+ base64, csv, u128). Also an explicit `[deps.bayan]` pin at `modules=["dist/bayan.cyr"]`, for the same one-packaging reason as `sigil` (see the `cyrius.cyml` comment). **daimon reads request bodies ONLY with `http_body_json` + `http_json_str` / `_int` / `_has` / `_text` (`src/http.cyr`)**, which use bayan's TYPED parser (`bayan_json_v_parse`). Never the flat `json_parse` / `json_get`. Its own contract says values keep their JSON escapes undecoded, and its scan misreads nested objects, which is why daimon's `jget` was removed at 2.3.2 (CHANGELOG 2.3.2, audit VULN-017). `json_escape_str` (output escaping, every control byte) lives in `src/error.cyr`. |
| `hashmap` | Hash map. `map_new()` = cstr keys; `map_new_str()` = `Str` struct keys; `map_u64_new()` = u64 inline keys (5.5.20). Pick at construction. |
| `process` | Fork, exec, waitpid |
| `fs` | File operations |
| `chrono` | Timestamps |

External (non-stdlib) deps used by daimon:

| Dep | Purpose |
|--------|--------|
| `sakshi` (2.5.2) | Structured logging/tracing — git-pinned via `[deps.sakshi]` in `cyrius.cyml`; resolved into `lib/sakshi.cyr` (gitignored) by `cyrius deps`. The 2.4.x line carries the daimon-class slot-array fix (`ts[2]` timespec → `i64[N]`; requires pin ≥ 6.2.1), arch-portable syscalls (x86_64 + aarch64), opt-in `sakshi_clock_recalibrate()`, and the agnos x86-TSC-calibration guard. **2.4.4** adds 128-bit trace-id support (`sakshi_trace_set_128`/`_hi`/`_lo`) that daimon's tracing (`src/trace.cyr`) adopts for full W3C `traceparent` round-trip. The `msg_len`-required call surface is unchanged since 2.0.0. |
| `bote` (3.3.13) | MCP core service. daimon hosts **13 builtin MCP tools** (`src/mcp_builtin.cyr`): bote's five `libro_*` audit tools (1.3.0), bote's `web_fetch` / `web_search` (1.4.0) and nein's six firewall tools (2.1.8). Vendored as the `dist/bote.cyr` bundle. daimon calls the handlers directly rather than bote's `dispatcher_dispatch`, keeping its own MCP registry and response shape. An external registration can never take a builtin's name (2.2.3). |
| `libro` (2.10.3) | Hash-linked audit chain (chain / merkle / query / retention / proof) backing bote's libro tools. daimon owns one `chain_new()` chain in `src/audit.cyr` (genesis-seeded at `daimon_audit_init`); the `daimon_audit` / `daimon_audit_agent` helpers append daimon's own lifecycle + security events (agent spawn/stop/exit, agent-channel faults `ipc.frame.*`, Host / Origin refusals `http.host.reject` / `http.origin.reject`, rate-limit, MCP SSRF-reject, external MCP call) to it, and the libro_* MCP tools read the same chain. Vendored `dist/libro.cyr`; pulls the `ct` / `keccak` / `random` / `slice` / `thread_local` / `sync` stdlib modules. |
| `majra` (2.9.1) | Event-sink dep of bote's bundle (`events_majra.cyr`). Present for compile-time resolution of the full bote bundle; daimon does not use it at runtime today. **Owns `ERR_IPC = 4`** — historically daimon renamed its own IPC error `ERR_IPC_FAULT` (1.3.0) to dodge this collision; at 1.4.2 every daimon error constant was namespaced `DAIMON_ERR_*` (now `DAIMON_ERR_IPC_FAULT`), so no daimon constant can collide with a vendored `ERR_*` and the rename satisfies cyrlint's `lint_error_enum_namespace` rule (6.4.51). |
| `samay` (1.1.3) | Task scheduler — the extraction of daimon's own `scheduler.cyr`/`cron.cyr`, which daimon 2.0.0 replaced with this library. `[deps.samay]` carries both a `path = "../samay"` (local dev checkout) and a `tag` (the release pin). samay has no scheduler-level "start": daimon's task start / complete live in `src/sched.cyr` (2.3.1). ⚠ `task_scheduler_complete_task` answers `Err(1)` both for an unknown id and for a refused transition, so look the task up first. |
| `nein` (1.7.0) | Firewall MCP tools (2.1.8, `dist/nein-mcp.cyr`). It is declared after `bote` and `sigil` in `cyrius.cyml` because its bundle leaves their symbols for the host to supply. The mutating half (`nein_allow` / `nein_deny`) is **gated shut** until caller authentication (roadmap 2.5.x). |
| `ai-hwaccel` (2.3.27) | samay's hardware-acceleration dependency (samay's own `[deps.ai-hwaccel]`), declared here since 2.0.0 so the full `dist/samay.cyr` bundle resolves at compile time. daimon calls nothing in it directly. Resolved from its tag with no `path` (2.4.0): with a `path`, the tag is inert and every build re-copies the sibling checkout, which leaked one mid-change into the lock at 2.3.4. |

**ADR-002 is invalid** — `lib/async.cyr` provides epoll-based cooperative async:
```cyrius
var rt = async_new();
async_spawn(rt, &my_handler, client_fd);
async_run(rt);  # event loop
```
Functions: `async_new`, `async_spawn`, `async_run`, `async_sleep_ms`, `async_read`, `async_await_readable`, `async_timeout`.

## Consumers

Every AGNOS agent, every consumer app, hoosh, agnoshi, aethersafha.

## Development Process

### P(-1): Scaffold Hardening (before any new features)

0. Read roadmap, CHANGELOG, and open issues — know what was intended before auditing what was built
1. Test + benchmark sweep of existing code
2. Cleanliness check — the gate under **Commands** below (vet, fmt, lint, three builds, `sh tests/test.sh`). ⚠ `cyrius check` is only a syntax check (`cyrius help`), not this gate
3. Get baseline benchmarks (`./scripts/bench-history.sh`)
4. Internal deep review — gaps, optimizations, security, logging/errors, docs
5. External security research — search for CVEs, 0-days, and vulnerability patterns relevant to daimon's attack surface (HTTP servers, Unix sockets, process supervisors, JSON parsers, file-based stores, bump allocators). Cross-reference against our code.
6. Security audit report — write findings to `docs/audit/{date}-security-audit.md` with severity, CVE references, affected code, and remediation steps. Roadmap any repair work found.
7. External research — domain completeness, missing capabilities, best practices, world-class accuracy
8. Cleanliness check — must be clean after review
9. Additional tests/benchmarks from findings
10. Post-review benchmarks — prove the wins
11. Documentation audit — ADRs, source citations, guides, examples (see Documentation Standards in first-party-standards.md)
12. Repeat if heavy

### Work Loop / Working Loop (continuous)

1. Work phase — new features, roadmap items, bug fixes
2. Cleanliness check (the gate under **Commands**)
3. Test + benchmark additions for new code
4. Run benchmarks (`./scripts/bench-history.sh`)
5. Internal review — performance, memory, security, throughput, correctness
6. External security research — if security-touching changes were made, search for relevant CVEs/0-days affecting the changed subsystems. Write findings to `docs/audit/` and roadmap any repair work.
7. Cleanliness check — must be clean after review
8. Deeper tests/benchmarks from review observations
9. Run benchmarks again — prove the wins
10. If review heavy → return to step 5
11. Documentation — update CHANGELOG, roadmap, docs, ADRs for design decisions, source citations for algorithms/formulas, update docs/sources.md, guides and examples for new API surface, verify recipe version in zugot
12. Version check — VERSION, cyrius.cyml, recipe (in zugot) all in sync
13. Return to step 1

### Task Sizing

- **Low/Medium effort**: Batch freely — multiple items per work loop cycle
- **Large effort**: Small bites only — break into sub-tasks, verify each before moving to the next. Never batch large items together
- **If unsure**: Treat it as large. Smaller bites are always safer than overcommitting

### Refactoring

- Refactor when the code tells you to — duplication, unclear boundaries, performance bottlenecks
- Never refactor speculatively. Wait for the third instance before extracting an abstraction
- Refactoring is part of the work loop, not a separate phase. If a review (step 5) reveals structural issues, refactor before moving to step 6
- Every refactor must pass the same cleanliness + benchmark gates as new code

### Key Principles

- Never skip benchmarks
- Own the domain — daimon IS the agent orchestration vocabulary
- Every type should have serde roundtrip tests (JSON via lib/bayan.cyr)
- All HTTP endpoints must validate input at the boundary
- Agent operations require explicit approval for sensitive actions
- Use `Result`/`Option` tagged unions for error handling
- Zero-crash in library code — no unguarded aborts
- Use accessor functions for struct fields
- **Fixed local arrays: use element-typed `var a: i64[N]` for slot arrays** (and `u8[N]` / `i32[N]` / `u32[N]` for sized byte/scalar buffers). Since cyrius 6.2.1, bare `var a[N]` is **N bytes in a function** (N i64 slots only at top level) — an address-taken `var a[N]` written via `store64(&a + i*8)` under-reserves and silently corrupts adjacent memory. This caused the 1.2.6 routing-404 bug; swept in 1.2.7. Before re-testing any "fixed" compiler footgun, **read the cyrius language CHANGELOG** — fixes there are often language changes, not silent codegen patches.
- Original Rust implementations are in git history at tags `0.5.0` / `0.6.0` (e.g. `git show 0.6.0:src/agent.rs`). The Cyrius port starts at `0.7.0`.

## Commands (verified at 2.4.1)

```sh
export CYRIUS_NO_WARN_SHADOW_LIB=1 CYRIUS_DCE=1   # what CI sets
cyrius build src/main.cyr build/daimon           # also: --agnos build/daimon-agnos, --aarch64 build/daimon-aarch64
cyrius test tests/<suite>.tcyr                   # one suite (17 suites, each includes its real src/ module)
sh tests/test.sh                                 # every suite + tests/smoke.sh (the linked binary over HTTP) + cyrius fuzz
cyrius vet src/main.cyr
cyrius fmt <file> --check                        # CI runs this on src/*.cyr tests/*.tcyr tests/*.bcyr tests/agnos/*.cyr fuzz/*
cyrius lint <file>                               # CI fails on any `warn`
./scripts/bench-history.sh                       # tests/daimon.bcyr; `cyrius bench tests/rag_ingest.bcyr` for the other two
./scripts/version-bump.sh <version>              # VERSION + src/config.cyr, only when the user names the version
sh tests/agnos/run.sh --release                  # AGNOS guest test on the released kernel, SHA-256 pinned (CI runs this)
sh tests/agnos/run.sh                            # ... on the sibling builds ../agnos/build/agnos, ../gnoboot/build/BOOTX64.EFI
rm -rf lib && cyrius deps                        # when cyrius.lock names files a clean checkout does not vendor (CI fails on it)
```

`cyrius fmt <file>` without `--check` rewrites the file in place. `cyrius check` is only a syntax check.
A suite prints its `N passed` line BEFORE it exits. Judge a run by its exit status and elapsed time,
not by a grepped summary (2.3.3: a suite printed 102/102 and then hung). aarch64 suites:
`cyrius build --aarch64 tests/<suite>.tcyr <out>` then `qemu-aarch64 <out>`. Under qemu, `agent.tcyr`
fails six checks that also fail on 2.3.2 there: QEMU applies no RLIMIT_AS, and qemu-user runs a
thread of its own.
`tests/agnos/run.sh` (2.4.0) builds `tests/agnos/*.cyr` and daimon for agnos, boots the PREBUILT
kernel (`AGNOS_KERNEL`, `GNOBOOT_EFI`; read, never written) under QEMU with its own image in
`build/agnos-guest/`, and reads `GUEST DONE` / `CLIENT DONE` from the serial log. `--release` (2.4.1)
downloads the agnos and gnoboot releases pinned in the script instead, into `build/agnos-release/`,
and refuses them unless their SHA-256 matches. Move a pin by changing the version and hash together.
Do not run the agnos repo's own harnesses from daimon: they write into `../agnos/build`, and a
sibling checkout may be another session's work in progress.
`cyrius deps` (and every build, which runs it first) rewrites `cyrius.lock` from what `lib/` holds,
leftovers included. CI resolves with `--no-lock` and verifies the committed lock (2.4.1), so a lock
that names a file a clean checkout does not vendor fails there.

## Conventions that bite (read before writing code)

- **Read the contract before you use a stdlib function** (`lib/*.cyr` header comments; the cyrius
  docs and CHANGELOG). Cyrius is not C or Rust. When daimon breaks, assume daimon's usage first, not
  the stdlib or compiler. A grep miss proves nothing: read the source.
- **`Str` is a fat pointer `{data, len}`.** Its data is NUL-terminated only when the maker made it
  so: `str_builder_build`, `str_clone`, `str_from_buf`, `str_cstr`, `str_from_int`. `str_cat` does
  not. A string literal passed to a `: Str` parameter is converted; a `str_data(...)` expression is
  not. At every C-string boundary (paths, syscalls, `map_new()` cstr keys, argv, audit details) use
  `str_cstr` or `path_join`. 2.2.3's CI failure was `str_data(str_cat(...))` handed to `is_dir`.
- **Request bodies**: `http_body_json` + `http_json_*` only (see the `bayan` row). They are already
  decoded, owned copies.
- **Retention**: anything a struct keeps from a request must be owned (`str_clone`). sandhi reuses the
  request buffer, and this caused the 1.2.5 / 2.1.6 / 2.2.1 leaks.
- **Agent processes**: only through `agent_spawn_with_limits` (`src/agent.cyr`), which:
  - makes the child lead its own process group, and die with daimon (PDEATHSIG);
  - closes inherited descriptors (`lib/net.cyr` sockets are not close-on-exec), except the channel
    on fd 3 and, with `--agent-output capture`, the output pipe on fds 1 and 2;
  - gives the child stdin from `/dev/null`;
  - resets SIGPIPE;
  - applies checked rlimits, and daimon's original descriptor limit;
  - reports exec failure synchronously.

  Signal an agent with `daimon_signal_tree` (its whole group).

  The executable comes from the agent's TYPE (`agnos-agent-<type>-agent` in `/usr/lib/agnos/agents`,
  `/opt/agnos/agents` or `serve --agents-dir`), **never from a request**.
- **The HTTP API has no authentication** until roadmap 2.5.x:
  - it binds 127.0.0.1 (`serve --listen` widens it), and while it does, a request must name a
    loopback host (`http_host_allowed`, in `handle_request`);
  - state changes are POST / DELETE only (`route_method`), and one from another site's page is
    refused on every route (`http_origin_foreign`, in `handle_request`);
  - agent and task control answer 403 to any request carrying `Origin` (`_route_refuse_browser`).

  Keep all of these for new routes.
- **The event loop (2.3.4, ADR-006)**: daimon is ONE thread running `server_loop` (`src/server.cyr`)
  over one epoll set: the listener, connections being read, agent channels and output pipes
  (`src/ipc.cyr` registers those). Upkeep runs on its tick (`agents_tick`: reap, advance stops).
  On agnos the same loop is polled (2.4.0, ADR-007).
  - **Do not start a thread.** It arms the stdlib's heap lock for every allocation, for good.
  - A handler runs on the loop, so **a handler that blocks holds everything**: channels, other
    clients, stops. Never wait in place. Wait for a process by deferring the answer
    (`server_defer`, as `api_agent_stop` does). Run work that waits on another server in a child
    (`server_detach`, as the MCP forwards and `web_fetch` do; on agnos too since 2.4.1, over
    `fork`#96). The child's memory is a copy: anything the call must record (an audit entry) is
    done before detaching. The child answers through `http_send_response` on the fd it is given.
  - **Every deadline reads `daimon_now_ms`**, never `clock_now_ms` (2.4.1). On agnos, a refused TSC
    calibration stops `clock_now_ms` for the whole boot; `daimon_now_ms` falls back to the tick.
  - End a program or suite with `sys_exit_group` / `syscall(SYS_EXIT_GROUP, …)`, the epilogue
    `lib/syscalls.cyr` prescribes (`sys_exit` ends only the calling thread).
  - The fork child (`_agent_child`) must not allocate: it runs only syscalls and stack buffers.
- **Every unit that includes `src/agent.cyr` must include `src/ipc.cyr`.** An undefined function is
  only a *warning* when no call reaches it, and a build error when one does.
- **cyrius has no adjacent-literal concatenation** (`"a" "b"` is a syntax error), and `cyrius fmt
  --check` / `cyrius lint` do not compile: build or run the suite after every edit.
- **AGNOS is the primary target.** Arch- and kernel-specific calls go through the shims in
  `src/syscalls.cyr` (`daimon_reap_code`, `daimon_signal`, `daimon_yield_ms`, …) and the agnos arms
  in `src/agent.cyr`, `src/ipc.cyr` and `src/server.cyr` (2.4.0, ADR-007): `spawn_path`#43, `kill`#16,
  `waitpid`#4, `chan_op`#97, `proclist`#99. For agnos syscalls read `agnos/kernel/core/syscall.cyr`,
  not the cyrius peer `lib/syscalls_x86_64_agnos.cyr`, which mirrors an older kernel.
  - **Never `sleep_ms` / `sys_nanosleep` on agnos**: both are `sleep_ms`#41, which disables
    preemption for the whole sleep, so the agents daimon waits for do not run. Wait with
    `daimon_yield_ms` (`pause`#14 until the deadline).
  - **Never write more than 1 KB at once on agnos** (2.4.1). A TCP receive ring there is 2048 bytes
    and `sock_send`#48 holds the CPU while it waits for room, so one larger write to a local process
    stops the machine. A pipe is a 4080-byte ring that returns short writes. Write with
    `daimon_write_all` (1 KB pieces, a yield between); `http_send_response` already does.
  - **Room is the kernel's**: 16 process slots and 16 channels for the machine, 32 fds per process,
    8 TCP slots for every end of every connection (one inside the machine takes two). `kill`#16 only
    sets a pending bit, so a child daimon forks or spawns cannot be made to exit.
  - **The `/bin/agnsh` slot is a foreground `execwait` child** (kybernet runs it to completion):
    something tested there does not behave as daimon in service would. `tests/agnos/` puts a launcher
    in that slot and runs everything under test as background processes.
  - **A gap in agnos is filed with agnos, not designed around as permanent**: a dated issue in the
    agnos repo's `docs/development/issues/`, in its format (cited lines, measurements, the ask, what
    daimon does meanwhile). daimon's interim is explicit, documented and audited (ADR-007's table).

## DO NOT

- **NEVER bump VERSION without the user's express permission.** Not as part of a
  feature, a fix, a lint repair, a doc update, or "the work loop" — never on your
  own initiative. The user owns versioning and release scheduling. If a change
  seems to warrant a version bump, *ask first*. (Work-loop step 12 below means
  keep VERSION/cyrius.cyml/recipe *consistent with each other*, NOT "bump VERSION
  yourself.")
  **When the user NAMES the version for the work** ("do 2.3.0 next", "cut it as
  2.2.3"), **that IS the permission**. Run `./scripts/version-bump.sh <version>`
  at once and do the release's work; do not ask again.
- **Do not commit or push** — the user handles all git operations
- **NEVER use `gh` CLI** — use `curl` to GitHub API only
- Do not add unnecessary dependencies
- Do not break backward compatibility without a major version bump
- Do not skip benchmarks before claiming performance improvements
- Do not blame the Cyrius stdlib, compiler or kernel for a defect in daimon's usage. Read the
  contract first; report what failed, with its output, not a guessed cause
- Do not raise, or work on, other projects (agnos, agnoshi, cyrius, …) unless they block daimon work

## Documentation Structure

```
Root files (required):
  README.md, CHANGELOG.md, CLAUDE.md, CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md, LICENSE

docs/ (required):
  architecture/overview.md — module map, data flow, consumers
  development/roadmap.md — completed, backlog, future, v1.0 criteria

docs/ (when earned):
  adr/ — architectural decision records (004: agent process control on an unauthenticated API)
  audit/ — security audit reports (ADR-003; required after security-touching work)
  guides/ — usage guides, integration patterns
  examples/ — worked examples
  standards/ — external spec conformance
  compliance/ — regulatory, audit, security compliance
  sources.md — source citations for algorithms/formulas (required for science/math crates)

docs/doc-health.md — the doc-currency ledger; update its rows when docs change.
docs/development/roadmap.md — OPEN work only; shipped work goes in CHANGELOG.md.
```

## CHANGELOG Format

Follow [Keep a Changelog](https://keepachangelog.com/). Performance claims MUST include benchmark numbers. Breaking changes get a **Breaking** section with migration guide.
