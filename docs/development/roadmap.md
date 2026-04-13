# Daimon Roadmap

## Completed (v0.7.0)

- [x] Port all core modules from Rust to Cyrius (9,724 LOC → 3,846 LOC)
  - [x] error, config, agent, supervisor, api, ipc, scheduler, memory, vector_store, rag, mcp, federation, edge, screen, logging
- [x] 24 HTTP API endpoints with full Rust parity
- [x] FederatedVectorStore — collection/replica management, cross-node search merge with dedup + re-ranking
- [x] Unix domain socket IPC — AgentIpc with bind/accept/send, length-prefixed wire protocol, ACK/NACK
- [x] CronScheduler — interval-based cron entries with validation
- [x] NodeCapacity — resource fitting, reserve/release, bin-packing scheduler
- [x] Federation cluster — Raft election (vote request/receive/step-down), agent placement scoring
- [x] Test suite (200 assertions / 26 groups), benchmark suite (16), fuzz harnesses (5)
- [x] P(-1) scaffold hardening + security audit (docs/audit/2026-04-13)
- [x] Modern Cyrius 4.2.0 toolchain (`cyrius build`, `cyrius deps`, `.cyrius-toolchain`)
- [x] CI/CD pipelines (GitHub Actions)

## Security Remediation (from audit 2026-04-13)

- [x] VULN-001: Content-Length parsing, Transfer-Encoding rejection (501), oversized payload (413)
- [x] VULN-002: `json_escape_str()` on all user-controlled strings in JSON responses
- [x] VULN-004: `pidfd_open()`/`pidfd_send_signal()` with `kill()` fallback (Linux 5.3+)
- [x] VULN-005: Agent memory dirs 0700 (was 0755)
- [x] VULN-006: `SO_PEERCRED` UID verification on Unix socket accept
- [x] VULN-008: `MAX_REQUEST_SIZE=65536`, Content-Length body reads, 413 response
- [x] VULN-010: `agent_spawn_with_limits()` with `setrlimit(RLIMIT_AS, RLIMIT_CPU)`
- [ ] VULN-009: Per-IP rate limiting with sliding window, 429 responses (deferred to v0.9.0)

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

- [x] All modules ported to Cyrius
- [x] Full HTTP API parity with Rust (24/24 endpoints)
- [x] Test coverage for all ported modules (200 assertions)
- [x] Benchmark baselines established
- [x] Security audit remediation (8/10 fixed, 1 accepted, 1 deferred)
- [ ] Documentation complete (API reference, architecture guide)
