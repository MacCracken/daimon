# Daimon Roadmap

## Completed

- [x] Initial scaffold (0.1.0)
- [x] Error types with axum IntoResponse
- [x] Config with serde + defaults
- [x] Module stubs for all subsystems
- [x] CI/CD pipelines

## Backlog

- [ ] Extract HTTP API handlers from `userland/agent-runtime/src/http_api/`
- [ ] Extract supervisor from `userland/agent-runtime/src/supervisor/`
- [ ] Extract IPC from `userland/agent-runtime/src/ipc.rs`
- [ ] Extract scheduler from `userland/agent-runtime/src/scheduler/`
- [ ] Extract federation from `userland/agent-runtime/src/federation/`
- [ ] Extract edge fleet from `userland/agent-runtime/src/edge/`
- [ ] Extract memory/vector/RAG stores
- [ ] Extract MCP dispatch from `userland/agent-runtime/src/mcp_server/`
- [ ] Extract screen capture/recording
- [ ] Integration tests (full HTTP round-trip)
- [ ] Benchmark suite (agent registration throughput, API latency, MCP dispatch)

## Future

- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Distributed tracing integration (OpenTelemetry)
- [ ] Agent migration between nodes

## v1.0 Criteria

- All modules extracted from monorepo and passing tests
- Full HTTP API parity with `userland/agent-runtime/`
- 80%+ test coverage
- Benchmark baselines established
- Documentation complete (API reference, architecture guide)
