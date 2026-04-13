# Daimon Roadmap

## Completed (v0.7.0)

- [x] Port core modules from Rust to Cyrius (9,724 LOC → 1,993 LOC)
  - [x] error → error codes + HTTP status mapping
  - [x] config → service configuration
  - [x] agent → agent lifecycle, process spawning, /proc helpers
  - [x] supervisor → circuit breaker, output capture, health monitoring, quotas
  - [x] api → HTTP API (synchronous TCP, port 8090)
  - [x] ipc → message bus, IPC message types
  - [x] scheduler → priority-aware task scheduling, state machine
  - [x] memory → per-agent key-value store (filesystem-backed)
  - [x] vector_store → cosine-similarity vector index
  - [x] rag → text chunking, bag-of-words embedding, retrieval pipeline
  - [x] mcp → MCP tool registry (builtin + external)
  - [x] federation → multi-node clustering, role tracking
  - [x] edge → edge fleet management, heartbeats, stats
  - [x] screen → capture permissions, rate limiting, recordings
  - [x] logging → sakshi integration

## Backlog

- [ ] Port Rust integration tests to Cyrius test suite
- [ ] Port Rust benchmarks to Cyrius bench suite
- [ ] Cron scheduler (CronScheduler from scheduler.rs)
- [ ] Firewall MCP tools (nein integration)
- [ ] JSON body parsing for all POST endpoints (agent name, MCP registration)
- [ ] Additional API endpoints: agent details, agent control, RAG ingest/query, scheduler submit

## Future

- [ ] Async HTTP API — when Cyrius async service patterns mature
- [ ] jnana integration — grounded knowledge queries backed by verified AGNOS science data
- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Distributed tracing integration (sakshi)
- [ ] Agent migration between nodes

## v1.0 Criteria

- [x] All modules extracted from monorepo and passing tests (Rust)
- [ ] Core modules ported to Cyrius
- [ ] Full HTTP API parity with Rust implementation
- [ ] Test coverage for all ported modules
- [ ] Benchmark baselines established (Cyrius)
- [ ] Documentation complete (API reference, architecture guide)
