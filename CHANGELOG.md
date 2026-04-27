# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [1.1.3] - 2026-04-27

### Added

- **`[release]` table** in `cyrius.cyml` — declares `bins = ["daimon"]` + `cross_bins = ["daimon-aarch64"]` as the canonical distribution list. `release.yml` continues to drive GitHub releases directly, but future tooling (`cyrius package`, ark) reads from this single source instead of duplicating the binary list.

### Changed

- **Lint warnings reduced 6 → 2** in `src/main.cyr`. Fixed long lines:
    - `rag_config_default` prompt template extracted to a `prompt_tmpl` local (was a 119-char `str_from(...)` inline at L1075).
    - `agent_ipc_accept_one` big-endian u32 length parse wrapped across 4 lines (was a single 119-char chained `load8 * 16777216 + ...` at L2823).
    - `api_mcp_call` inline JSON literal split into 3 `str_builder_add_cstr` calls (was a single 122-char literal at L3393).
    - `ip_to_cstr` dotted-decimal print block reformatted with one statement per line (the `if (val >= 100) {...}` and `elif (val >= 10) {...}` were single 120/123-char lines at L3918-3919).
- Remaining 2 warnings are confirmed cyrius 5.7.12 linter false positives — see "Known issues" below. CI lint step stays `continue-on-error: true` until upstream fixes land.

### Known issues

- **`cyrius lint` false positive: `unclosed braces at end of file`**. The linter doesn't track string-literal state, so daimon's str-builder JSON pattern (`"{\"id\":\""` openings paired with `"}"` closings across separate calls) registers as 30 `{` vs 37 `}` overall and triggers the EOF brace warning. File parses cleanly (`cyrius check --with-deps` ok; 200/200 tests pass).
- **`cyrius lint` false positive: `trailing whitespace` at line 4177**. Comment line `# Parse remaining args: port number or --async` has zero trailing whitespace under `cat -A`, `git diff --check`, and `owl -A`. Persists after editing the comment text. Most likely a linter line-counter bug after multi-line replacements earlier in the file. Reproduces on cyrius 5.7.12.
- **`cyrius fmt` truncates `src/main.cyr`** mid-string at line 4168 on cyrius 5.7.12. Discovered when piping `cyrius fmt | cyim --write src/main.cyr` produced a 4134-line truncated file (was 4210). Hence `cyrius fmt --check` is not gated in CI; reverted via `git checkout` and unaffected since.

### Deferred (Tier 2 — separate release)

- `lib/sandhi.cyr` adoption to replace daimon's hand-rolled HTTP server (`http_parse_method`, `http_send_response`, etc., L2979-3556). Sandhi brings HTTP/2, SSE, JSON-RPC + MCP-over-HTTP (`sandhi_rpc_mcp_call`) — would also unstub daimon's external MCP forwarding in sync mode (currently returns the "tool dispatch not available in sync mode" error reformatted in this patch).
- `lib/atomic.cyr` for circuit-breaker counters in `cb_*` (L477-548): reviewed and explicitly skipped — both sync and async (epoll-cooperative) HTTP modes are single-threaded, so there is no concurrent reader/writer to atomicize. Adding atomics would solve a non-problem.
- `lib/sankoch.cyr` for memory-store compaction: deferred to its own release with a benchmark cycle.

## [1.1.2] - 2026-04-27

### Changed

- **Cyrius toolchain bump**: `4.5.0` → `5.7.12`. Build remains clean (`cyrius check --with-deps`), test suite holds (200 passed / 0 failed), binary builds (`build/daimon`, ~263 KB statically linked).
- **Manifest migration**: `cyrius.toml` → `cyrius.cyml` (5.0.0 breaking; `cyrius update` semantics). `version` now resolves from the `VERSION` file via `${file:VERSION}`. `output` moved under `build/` to match the modern layout used by vidya 2.3.0.
- **`sakshi` is no longer stdlib**: dropped from `[deps] stdlib`, added as `[deps.sakshi]` git pin (`tag = "2.0.0"`). After `cyrius deps`, `lib/sakshi.cyr` is a symlink into `~/.cyrius/deps/sakshi/2.0.0/dist/sakshi.cyr`.
- **`math` added to stdlib deps**: `f64_sqrt` (used by `vector_normalize` and `cosine_similarity`) needed `lib/math.cyr` registered explicitly so `cyrius vet` resolves it; build was implicitly pulling it in via stdlib auto-prepend, vet now reports `2 deps, 0 untrusted, 0 missing`.
- `cyrius.lock` now committed (5.7.8 made lockfile-on-by-default).
- CLAUDE.md: updated Cyrius pin reference (4.2.0 → 5.7.12), added note that `lib/sandhi.cyr` is the recommended HTTP path for new server work, documented `hashmap` key-type variants and external sakshi dep.

### CI / Release

- **CI overhauled** (`.github/workflows/ci.yml`) modeled on the modern agnostik 5.7.x shape: toolchain version pulled from `cyrius.cyml` (was `.cyrius-toolchain`); added `cyrius deps --verify`, `cyrius vet`, DCE build (`CYRIUS_DCE=1`), ELF magic check, best-effort aarch64 cross-build, per-test loop with discrete failure visibility, `cyrius bench` step, security-scan job (no `sys_system`, no writes to `/etc|/bin|/sbin`, no ≥64 KB stack buffers — comment-aware), and a docs job that checks 12 required files + version-in-CHANGELOG consistency.
- **`cyrius lint` runs as advisory** (`continue-on-error: true`) — daimon has 6 standing warnings (4 long lines + 1 false-positive brace from JSON-in-string-literal at line 3393). Tier-3 cleanup will eliminate them and flip lint to fail-on-warn.
- **`cyrius fmt --check` not gated**: cyrius 5.7.12 fmt has a truncation bug on daimon's `src/main.cyr` (cuts mid-string at line 4168). Skipping the gate until the upstream fix lands.
- **Release workflow rewritten** (`.github/workflows/release.yml`) to mirror agnostik: accepts both `v1.2.3` and `1.2.3` tag styles, semver-shape verification, source tarball + cross-arch binaries + `SHA256SUMS` archive, per-version changelog extraction via awk, pre-release detection for `0.x` tags.
- **`scripts/version-bump.sh`** simplified — only writes `VERSION` now, since `cyrius.cyml` resolves `[package].version` from it via `${file:VERSION}`.
- **README.md / CONTRIBUTING.md** updated for `cyrius.cyml` + new pin.

## [1.1.1] - 2026-04-13

### Changed

- Roadmap: unblocked nein-core firewall work. [nein](https://github.com/MacCracken/nein) v0.1.0 Cyrius port shipped 13 modules (rule/table/chain/set/nat/bridge/engine/mesh/geoip/policy/builder/firewall/validate) — daimon can now depend on nein directly. Nein's own `mcp` module stays gated on bote.

## [1.1.0] - 2026-04-13

### Added

- **Async HTTP server** — `serve --async` flag enables epoll-based cooperative concurrency via `lib/async.cyr`. Handles multiple connections per accept cycle with batched async_run. Both sync and async modes share the same request handler and security controls.
- Documentation: architecture overview, API guide with all 24 endpoints, quickstart guide, 3 ADRs (port rationale, HTTP mode, security process).
- Vendored `lib/async.cyr` (epoll cooperative runtime), `lib/http.cyr` (HTTP client), `lib/thread.cyr`, `lib/callback.cyr` via updated `cyrius.toml` deps.

### Changed

- Removed `rust-old/` directory (16 GB, 9,724 LOC Rust source + build cache). Rust history available in git pre-v0.7.0 tags.
- Refactored serve loop: request handling extracted to `handle_request(cfd)`, server setup to `server_bind(port)` — shared by both sync and async modes.
- SECURITY.md updated for v1.x supported versions, Cyrius-specific scope.
- CLI: `serve [port] [--async]` — async mode optional, sync remains default.

## [0.7.0] - 2026-04-13

Complete rewrite from Rust to Cyrius. 9,724 LOC Rust → 4,141 LOC Cyrius. Binary: 181 KB (was 4.0 MB). Zero external dependencies.

### Added

- **15 modules ported** with full API parity: error, config, agent, supervisor, memory, vector_store, rag, mcp, screen, scheduler, federation, edge, ipc, api, logging.
- **24 HTTP API endpoints** — synchronous TCP server on port 8090:
  - `/v1/health` — service health
  - `/v1/agents` (GET/POST), `/v1/agents/{id}` (GET) — agent lifecycle
  - `/v1/mcp/tools` (GET/POST), `/v1/mcp/tools/{name}` (DELETE), `/v1/mcp/call` (POST) — MCP tool dispatch
  - `/v1/rag/ingest` (POST), `/v1/rag/query` (POST) — RAG pipeline
  - `/v1/edge/nodes` (GET/POST), `/v1/edge/nodes/{id}` (GET), `…/heartbeat` (POST), `…/decommission` (POST), `/v1/edge/stats` — edge fleet
  - `/v1/scheduler/tasks` (GET/POST), `…/{id}` (GET), `…/{id}/cancel` (POST), `/v1/scheduler/nodes` (POST), `/v1/scheduler/schedule` (POST), `/v1/scheduler/stats` — task scheduling
  - `/v1/metrics` — aggregate metrics
- **Scheduler**: NodeCapacity with resource fitting + bin-packing, schedule_pending with assignment decisions, preempt_check, tasks_for_node, CronScheduler with interval-based entries + validation, stats aggregation.
- **Federation**: cluster management, heartbeat health tracking (online/suspect/dead), Raft-like election (start_election, receive_vote_request, receive_vote, become_coordinator, step_down), 4-factor weighted node scoring (resource/locality/load/affinity), agent placement, cluster stats.
- **Edge fleet**: register with validation (empty name, duplicate, fleet-full), heartbeat, health check (suspect/offline thresholds), decommission, list with status filter, stats.
- **FederatedVectorStore**: collection/replica management, cross-node search merge with dedup + re-ranking, remove_node, stats.
- **IPC**: Unix domain socket AgentIpc (bind/accept/send, length-prefixed wire protocol, ACK/NACK, connection limits), message bus (named routing + broadcast + direct send), RPC registry.
- **Memory store**: CRUD with atomic write (tmp+rename), list_keys, list_by_tag, clear, usage_bytes, key validation + sanitization.
- **RAG pipeline**: ingest_text (chunk + embed + index), query_text (embed + search + format context).
- **Vector store**: cosine similarity, brute-force search with ranking, normalize_vec.
- **Agent lifecycle**: start/stop/pause/resume with race-free pidfd signal delivery, /proc resource monitoring (VmRSS, CPU time, fd count, thread count), resource limits on spawned processes.
- **Supervisor**: circuit breaker (Closed→Open→HalfOpen), output capture ring buffer, resource quotas, health tracking.
- **MCP**: tool registry (builtin + external) with manifest, register, deregister, validate_callback_url.
- **Screen capture**: permission manager with rate limiting, recording sessions (active/paused/stopped).
- CLI: `serve [port]`, `version`, `help`.
- Test suite: 200 assertions / 26 test groups.
- Benchmark suite: 16 benchmarks with Rust comparison (BENCHMARKS.md).
- Fuzz harnesses: 5 (circuit_breaker, memory_keys, scheduler_fsm, vector_store, mcp_registry).
- Security audit: docs/audit/2026-04-13-security-audit.md — 10 findings, 9 fixed, 1 accepted risk.

### Security

- **VULN-001**: Content-Length validation, Transfer-Encoding rejection (501), 413 Payload Too Large. Prevents request smuggling.
- **VULN-002**: `json_escape_str()` on all user-controlled strings in JSON responses. Prevents JSON injection.
- **VULN-004**: `pidfd_open()`/`pidfd_send_signal()` with `kill()` fallback. Prevents PID reuse race.
- **VULN-005**: Agent memory directories 0700 (was 0755).
- **VULN-006**: `SO_PEERCRED` UID verification on Unix socket accept. Prevents unauthorized IPC.
- **VULN-008**: `MAX_REQUEST_SIZE=65536`, Content-Length body reads. Prevents oversized request DoS.
- **VULN-009**: Per-IP rate limiting — 120 req/min sliding window, 429 Too Many Requests.
- **VULN-010**: `agent_spawn_with_limits()` with `RLIMIT_AS` + `RLIMIT_CPU`. Prevents agent resource exhaustion.
- HTTP query parameter bounds checking. Prevents buffer over-read.
- Empty path segment handling (`/v1/agents/` → 404).

### Changed

- **Language**: Rust → Cyrius. Rust source removed in v1.0.1.
- **Toolchain**: Cyrius 4.2.0 (pinned in `.cyrius-toolchain`).
- **Build**: `cargo build` → `cyrius build src/main.cyr build/daimon`.
- **HTTP**: Async (tokio/axum) → synchronous (raw TCP sockets).
- **Dependencies**: 193 crate dependencies → 17 Cyrius stdlib modules + 0 external.
- **Binary**: 4.0 MB → 181 KB (96% smaller).

### Breaking

- Language changed from Rust to Cyrius. Consumers must use Cyrius 4.2.0+ to build.
- HTTP server is synchronous (single-threaded). No concurrent request handling.
- MCP tool call forwarding returns error stub — blocked on bote Cyrius port.
- Firewall MCP tools not available — blocked on nein Cyrius port.

## [0.6.0] - 2026-04-03

### Added

- `http-forward` feature gate — external MCP tool forwarding via reqwest is now opt-in.
- `Serialize`/`Deserialize` on 8 previously non-serializable types.
- Input validation: positive CPU/memory on scheduler node registration, cron bounds.
- 13 new tests (305 total), 4 new benchmarks (19 total).

### Fixed

- **Security**: External MCP tool-not-found → 400 (was leaking 404).
- **Security**: Firewall table filter exact match (was substring).
- **Safety**: `setrlimit` return values checked, scheduler `.get()` instead of index, RpcRouter mutex handling.

### Changed

- Binary size: 12 MB → 4.0 MB (−64%) default, 8.2 MB (−32%) with http-forward.
- Dependencies: 354 → 193 (−45%). Dropped anyhow, async-trait.
- `Supervisor::check_health` now generic.

## [0.5.0] - 2026-03-26

### Added

- Full axum HTTP API router with 20+ endpoints.
- Integration test suite (28 tests), benchmark suite (15 benchmarks).

### Fixed

- Restrictive CORS, timestamp correctness, cron time matching, socket permission logging.

## [0.1.0] - 2026-03-25

### Added

- Initial scaffold — all modules extracted from agnosticos monorepo.
