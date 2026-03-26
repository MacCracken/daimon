# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

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
