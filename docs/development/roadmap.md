# Daimon Roadmap

## Completed (v0.7.0)

- [x] Full Rust → Cyrius port (9,724 LOC → 4,141 LOC, 15 modules, 24 endpoints)
- [x] Test suite (200 assertions / 26 groups), benchmarks (16), fuzz harnesses (5)
- [x] Security audit + remediation (9/10 fixed, 1 accepted risk)
- [x] Modern Cyrius 4.2.0 toolchain, CI/CD pipelines

## Security Gates (trigger-based)

- [ ] VULN-007: Bump allocator memory zeroing — **MUST fix before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted federation, external MCP callbacks (bote). Remediation: per-agent arena allocators with zero-on-reset.

## Blocked on Upstream Ports

- [ ] Firewall MCP tools — blocked on [nein](https://github.com/MacCracken/nein) Cyrius port
- [ ] MCP tool hosting (bote re-exports) — blocked on [bote](https://github.com/MacCracken/bote) Cyrius port
- [ ] MCP tool call forwarding — blocked on bote + HTTP client library

## Completed (v1.1.0)

- [x] Async HTTP mode — `serve --async` using `lib/async.cyr` epoll runtime (ADR-002)
- [x] Documentation: architecture overview, API guide, quickstart, 3 ADRs
- [x] Removed `rust-old/`, vendored async/http/thread/callback stdlib modules

## Future (v1.2.0+)
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
- [x] Security audit remediation complete (9/10 fixed, 1 gated)
- [x] Documentation complete (architecture overview, API guide, quickstart, 3 ADRs, security audit)
