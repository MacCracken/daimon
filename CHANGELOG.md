# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

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
