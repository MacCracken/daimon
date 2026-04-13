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

### Immediate (before v0.8.0)

- [ ] VULN-001: Parse `Content-Length`, reject `Content-Length` + `Transfer-Encoding` conflicts, reject duplicates
- [ ] VULN-002: Add `json_escape_str()` for all user-controlled strings in JSON responses

### Next Sprint

- [ ] VULN-004: Use `pidfd_open()`/`pidfd_send_signal()` for race-free agent signal delivery (Linux 5.3+)
- [ ] VULN-006: `SO_PEERCRED` verification on Unix socket accept
- [ ] VULN-008: Explicit `MAX_REQUEST_SIZE`, `Content-Length`-based body reads, 413 responses, read timeout

### Backlog

- [ ] VULN-005: `O_NOFOLLOW | O_CREAT | O_EXCL` on tmp file writes, 0700 agent dirs
- [ ] VULN-009: Per-IP rate limiting with sliding window, 429 responses
- [ ] VULN-010: `setrlimit(RLIMIT_AS, RLIMIT_CPU)` on spawned agent processes

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
- [ ] Security audit remediation complete (VULN-001 through VULN-010)
- [ ] Documentation complete (API reference, architecture guide)
