# Daimon Roadmap

## Backlog

- [ ] Port core modules from Rust to Cyrius (see `rust-old/src/`)
  - [ ] error.rs → error handling (tagged unions)
  - [ ] config.rs → configuration
  - [ ] agent.rs → agent lifecycle
  - [ ] supervisor.rs → process supervision, circuit breaker
  - [ ] api.rs → HTTP API (synchronous, epoll-based)
  - [ ] ipc.rs → IPC (Unix domain sockets, message bus, RPC)
  - [ ] scheduler.rs → task scheduling, cron triggers
  - [ ] memory.rs → agent persistent key-value memory
  - [ ] vector_store.rs → embedded vector store
  - [ ] rag.rs → RAG pipeline
  - [ ] mcp.rs → MCP tool dispatch
  - [ ] federation.rs → multi-node federation
  - [ ] edge.rs → edge fleet management
  - [ ] screen.rs → screen capture
  - [ ] firewall.rs → firewall MCP tools
  - [ ] logging.rs → structured logging (sakshi)

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
