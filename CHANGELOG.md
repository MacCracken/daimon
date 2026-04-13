# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.7.0] - 2026-04-13

### Added

- **Cyrius port**: Full rewrite from Rust to Cyrius (9,724 LOC Rust → 1,993 LOC Cyrius).
- All 14 core modules ported: error, config, agent, supervisor, memory, vector_store, rag, mcp, screen, scheduler, federation, edge, ipc, api.
- Synchronous HTTP API server on port 8090 with JSON responses.
- API endpoints: `/v1/health`, `/v1/agents` (GET/POST), `/v1/mcp/manifest`, `/v1/edge/stats`, `/v1/metrics`.
- CLI: `serve [port]`, `version`, `help` commands.
- Circuit breaker with Closed → Open → HalfOpen state machine.
- Output capture ring buffer for agent stdout/stderr.
- Per-agent key-value memory store with atomic writes (tmp+rename).
- In-memory cosine-similarity vector index with brute-force search.
- RAG pipeline: text chunking, bag-of-words embedding, context formatting.
- MCP tool registry (builtin + external) with manifest generation.
- Screen capture permission manager with rate limiting.
- Priority-aware task scheduler with state machine transitions.
- Federation node management with Raft-like role tracking.
- Edge fleet manager with heartbeat tracking and stats aggregation.
- Message bus for agent IPC with named routing and broadcast.
- `/proc` helpers for agent resource monitoring (VmRSS, fd count, thread count).
- Binary size: **113 KB** (vs 4.0 MB default Rust build — 97% smaller).

### Changed

- **Language**: Rust → Cyrius. Rust source preserved in `rust-old/`.
- **HTTP**: Async (tokio/axum) → synchronous (raw TCP sockets via epoll).
- **Build**: `cargo build` → `cat src/main.cyr | cc3 > build/daimon`.
- **Dependencies**: 193 crate dependencies → 17 Cyrius stdlib modules + 0 external deps.

## [0.6.0] - 2026-04-03

### Added

- `http-forward` feature gate — external MCP tool forwarding via reqwest is now opt-in, keeping the default binary lean.
- `Serialize`/`Deserialize` on 8 previously non-serializable types: `RagPipeline`, `OutputCapture`, `EdgeFleetManager`, `McpHostRegistry` (fallback), `TaskScheduler`, `CronScheduler`, `RpcRegistry`, `VectorIndex`.
- `#[must_use]` on `IpcMessage::new()`, `TaskScheduler::new()`, `CronScheduler::new()`.
- `#[non_exhaustive]` on `VectorIndex` and `RagPipeline`.
- Input validation: positive CPU/memory on scheduler node registration, cron entry hour (0–23) and minute (0–59) bounds.
- 13 new tests (305 total): serde roundtrips for newly serializable types, cron validation, boundary validation integration tests.
- 4 new benchmarks (19 total): `rag_query_50_docs` (30 µs), `edge_heartbeat_100_nodes` (8 µs), `edge_stats_500_nodes` (765 ns), `federation_score_100_nodes` (1.7 µs).

### Fixed

- **Security**: External MCP tool-not-found now returns 400 Bad Request (`InvalidParameter`) instead of leaking 404 (`AgentNotFound`).
- **Security**: Firewall table filter used substring `.contains()` match — replaced with exact match to prevent unintended rule inclusion.
- **Safety**: `setrlimit` return values in `apply_rlimits()` are now checked and logged on failure.
- **Safety**: Scheduler `schedule_pending()` replaced direct HashMap indexing (`self.tasks[&id]`) with `.get()` to prevent potential panics.
- **Safety**: `RpcRouter` mutex locks replaced `.expect()` with `map_err`/`match` — no more panics in library code on lock poisoning.
- **Correctness**: `cargo fmt` and `cargo clippy` violations resolved (collapsible-if in firewall, benchmark type rename).
- **Correctness**: Benchmark file updated for `McpHostRegistry` rename and missing `mcp_handlers` field.

### Changed

- **Binary size**: Default binary 12 MB → **4.0 MB** (−64%) by feature-gating reqwest behind `http-forward`.
- **Binary size**: With `http-forward`, 12 MB → **8.2 MB** (−32%) by switching TLS from aws-lc-rs to ring.
- **Dependencies**: Dropped `anyhow` (replaced with crate's own `Result` in binary).
- **Dependencies**: Dropped `async-trait` (native async traits, edition 2024).
- **Dependencies**: `reqwest` moved to optional, default-features disabled, uses `rustls-no-provider` + ring.
- **Dependencies**: Default dependency count 354 → 193 (−45%).
- `Supervisor::check_health` now generic (`<A: AgentControl>`) instead of `dyn` dispatch.
- `http-forward` included in the `full` feature set.

### Breaking

- `reqwest` is no longer a default dependency. Consumers calling external MCP tools must enable the `http-forward` feature. Without it, external tool calls return an error message indicating the feature is required.
- `Supervisor::check_health` signature changed from `&dyn AgentControl` to generic `<A: AgentControl>`. Callers passing trait objects must switch to concrete types or generics.

## [0.5.0] - 2026-03-26

### Added

- Full axum HTTP API router (`api.rs`) with 20+ endpoints: health, agents, MCP tools, RAG ingest/query, edge fleet, scheduler, metrics.
- Integration test suite — 28 HTTP round-trip tests covering all API subsystems.
- Benchmark suite — 15 benchmarks covering agent registration throughput, MCP dispatch, API latency, vector search, RAG ingest, scheduler scheduling, and edge fleet registration.

### Fixed

- **Security**: Replaced permissive CORS (`CorsLayer::permissive()`) with restrictive default (`CorsLayer::new()`).
- **Correctness**: `Agent::handle()` now returns actual `created_at` and `started_at` timestamps instead of always using `Utc::now()`.
- **Correctness**: Scheduler cron time matching now uses `chrono::Timelike` instead of string formatting with `unwrap_or(0)`.
- **Safety**: Removed `.parse().unwrap()` from `FederationConfig::Default` — uses `SocketAddr::from()` directly.
- **Robustness**: IPC `RpcRouter` mutex locks use `.expect()` with descriptive messages instead of `.unwrap()`.
- **Robustness**: IPC socket permission failures are now logged instead of silently ignored.
- **Robustness**: IPC `MessageBus::publish()` logs warnings on dropped messages instead of silently discarding.
- **Robustness**: API serialization failures in list endpoints now log warnings instead of silently dropping items.
- **Robustness**: Logging initialization warns via stderr when `DAIMON_LOG` contains an invalid filter directive.

### Changed

- **Dependencies**: axum 0.7 → 0.8 (route params now use `{param}` syntax).
- **Dependencies**: reqwest 0.12 → 0.13 (switched from native-tls to rustls).
- **Dependencies**: nix 0.30 → 0.31.
- Added `ISC`, `MIT-0`, `CDLA-Permissive-2.0` to allowed licenses in `deny.toml` (required by rustls/aws-lc chain).

## [0.1.0] - 2026-03-25

### Added

- Initial scaffold — api, agent, supervisor, ipc, scheduler, federation, edge, memory, vector_store, rag, mcp, screen, config modules.
