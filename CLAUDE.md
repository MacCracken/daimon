# Daimon — Claude Code Instructions

## Project Identity

**Daimon** (Greek: δαίμων — guiding spirit) — AGNOS agent orchestrator

- **Type**: Service binary + library
- **Language**: Cyrius (ported from 9,724 LOC Rust)
- **Purpose**: AGNOS agent orchestrator — HTTP API, supervisor, IPC, scheduler, federation, edge fleet, memory, MCP dispatch (port 8090)
- **License**: GPL-3.0-only
- **Cyrius**: 5.7.12 (pinned in `cyrius.cyml`)
- **Version**: CalVer (see VERSION file)
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
| `thread` | Clone-based threads, mutex, MPSC channels |
| `net` | TCP sockets (connect, listen, accept, read, write) |
| `sandhi` | **In use as of 1.1.4** — drives both sync (`sandhi_server_run`) and async (`sandhi_server_recv_request` + smuggling checks inline) HTTP paths. `http_*` shims in `src/main.cyr` are sandhi-backed. Daimon does NOT use sandhi's HTTP/2 / SSE / TLS / RPC modules yet — `sandhi_rpc_mcp_call` is the planned hook for unstubbing external MCP forwarding (see `api_mcp_call`). |
| `json` | JSON parse/emit |
| `hashmap` | Hash map. `map_new()` = cstr keys; `map_new_str()` = `Str` struct keys; `map_u64_new()` = u64 inline keys (5.5.20). Pick at construction. |
| `process` | Fork, exec, waitpid |
| `fs` | File operations |
| `chrono` | Timestamps |

External (non-stdlib) deps used by daimon:

| Dep | Purpose |
|--------|--------|
| `sakshi` (2.0.0) | Structured logging/tracing — git-pinned via `[deps.sakshi]` in `cyrius.cyml`; resolved into `lib/sakshi.cyr` as a symlink by `cyrius deps`. |

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
2. Cleanliness check: `cyrius check` (fmt + lint + test + build)
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
2. Cleanliness check: `cyrius check`
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
- Every type should have serde roundtrip tests (JSON via lib/json.cyr)
- All HTTP endpoints must validate input at the boundary
- Agent operations require explicit approval for sensitive actions
- Use `Result`/`Option` tagged unions for error handling
- Zero-crash in library code — no unguarded aborts
- Use accessor functions for struct fields
- Original Rust implementations available in git history (pre-v0.7.0 tags)

## DO NOT

- **Do not commit or push** — the user handles all git operations
- **NEVER use `gh` CLI** — use `curl` to GitHub API only
- Do not add unnecessary dependencies
- Do not break backward compatibility without a major version bump
- Do not skip benchmarks before claiming performance improvements

## Documentation Structure

```
Root files (required):
  README.md, CHANGELOG.md, CLAUDE.md, CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md, LICENSE

docs/ (required):
  architecture/overview.md — module map, data flow, consumers
  development/roadmap.md — completed, backlog, future, v1.0 criteria

docs/ (when earned):
  adr/ — architectural decision records
  guides/ — usage guides, integration patterns
  examples/ — worked examples
  standards/ — external spec conformance
  compliance/ — regulatory, audit, security compliance
  sources.md — source citations for algorithms/formulas (required for science/math crates)
```

## CHANGELOG Format

Follow [Keep a Changelog](https://keepachangelog.com/). Performance claims MUST include benchmark numbers. Breaking changes get a **Breaking** section with migration guide.
