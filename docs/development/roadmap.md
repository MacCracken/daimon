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

## Completed (v0.7.0 continued)

- [x] FederatedVectorStore — collection/replica management, cross-node search merge with dedup + re-ranking, stats
- [x] Unix domain socket IPC — AgentIpc with bind/accept/send, length-prefixed wire protocol, ACK/NACK, connection limits

## Backlog

_(empty — all portable features complete)_

## Blocked on Upstream Ports

- [ ] Firewall MCP tools — blocked on [nein](https://github.com/MacCracken/nein) Cyrius port
- [ ] MCP tool hosting (bote re-exports) — blocked on [bote](https://github.com/MacCracken/bote) Cyrius port
- [ ] MCP tool call forwarding — blocked on bote + HTTP client library

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
