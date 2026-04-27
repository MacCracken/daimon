# Daimon Roadmap

## Completed (v0.7.0)

- [x] Full Rust → Cyrius port (9,724 LOC → 4,141 LOC, 15 modules, 24 endpoints)
- [x] Test suite (200 assertions / 26 groups), benchmarks (16), fuzz harnesses (5)
- [x] Security audit + remediation (9/10 fixed, 1 accepted risk)
- [x] Modern Cyrius 4.2.0 toolchain, CI/CD pipelines

## Security Gates (trigger-based)

- [ ] VULN-007: Bump allocator memory zeroing — **MUST fix before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted federation, external MCP callbacks (bote). Remediation: per-agent arena allocators with zero-on-reset.

## Blocked on Upstream Ports

- [ ] Firewall MCP tools — [nein](https://github.com/MacCracken/nein) core port complete (v0.1.0, 13 modules); MCP tool wiring still pending nein's own `mcp` module which is blocked on bote
- [ ] MCP tool hosting (bote re-exports) — blocked on [bote](https://github.com/MacCracken/bote) Cyrius port
- [x] ~~MCP tool call forwarding — blocked on bote + HTTP client library~~ — HTTP-client side unblocked by sandhi (`sandhi_rpc_mcp_call` available since Cyrius 5.7.0). Wiring is now a daimon-side feature, see v1.1.5 below.

## Completed (v1.1.0)

- [x] Async HTTP mode — `serve --async` using `lib/async.cyr` epoll runtime (ADR-002)
- [x] Documentation: architecture overview, API guide, quickstart, 3 ADRs
- [x] Removed `rust-old/`, vendored async/http/thread/callback stdlib modules

## Completed (v1.1.4)

- [x] HTTP server migrated to `lib/sandhi.cyr` — sync via `sandhi_server_run`, async via `sandhi_server_recv_request` + inline smuggling checks. ≈580 LOC of hand-rolled `http_*` parse/send replaced; `src/main.cyr` net −164 LOC.
- [x] VULN-001 strengthened — sandhi's strict RFC 7230 §3.3.2 Content-Length parse closes the loose-digit CL.CL sub-vector.
- [x] Slowloris bounded — sandhi's 30s `SO_RCVTIMEO` default replaces daimon's previous "no timeout" sock_recv.
- [x] Sandhi-migration security audit — `docs/audit/2026-04-27-sandhi-migration.md`.
- [x] Six pre-existing `sakshi_info`/`warn`/`error` calls fixed (missing `msg_len` arg since the sakshi 2.0.0 stdlib bump — surfaced by the migration smoke test).

## v1.1.5 — sandhi follow-ups

- [ ] **External MCP forwarding via `sandhi_rpc_mcp_call`** — replace the `api_mcp_call` `"tool dispatch not available in sync mode"` stub at `src/main.cyr:3393`. Requires extending `McpToolDescription` with an `endpoint_url` field, updating `api_mcp_register` to capture and validate it (HTTP/HTTPS only — no `file://` etc.; reuse `validate_callback_url` semantics if applicable), and wiring `sandhi_rpc_mcp_call` in the dispatcher. Add a roundtrip test against a fake MCP server. Surfaces in CHANGELOG 1.1.4 "Deferred".
- [ ] **Lower sandhi `idle_ms` below the 30s default** — `serve` calls `sandhi_server_run` with `opts = 0` so the slowloris timeout sits at sandhi's 30 000 ms default. Collect a baseline of legitimate-client P99 request durations from a 1.1.4 production soak, then thread `sandhi_server_options_new()` + `sandhi_server_options_idle_ms(opts, N)` + `sandhi_server_run_opts(...)` with N ≈ 5 000 ms. Trade-off documented in `docs/audit/2026-04-27-sandhi-migration.md` § VULN-008.
- [ ] **Collapse `serve_async` to `sandhi_server_run_opts`** once a Cyrius stdlib patch enforces `sandhi_server_options_max_conns`. Sandhi is at **1.0.0** (folded into Cyrius 5.7.0 stdlib as `lib/sandhi.cyr`, sandhi repo now in maintenance mode); concurrent server-accept was deliberately deferred at 0.8.0 in favour of nailing HTTP/2 + client connection pool first — the right call given how much P0 / P1 hardening (smuggling, slowloris, strict CL, ALPN, HPACK) shipped through 0.9.x on a single-threaded server. The hook is already in place: `sandhi_server_options_max_conns(opts, n)` and `sandhi_server_run_opts(...)` are both public; only the enforcement path is reserved. When a stdlib patch wires it up, daimon's `serve_async` (epoll loop + per-call buf alloc + inline smuggling-check duplication) collapses into one `sandhi_server_run_opts(...)` call shared with sync. Track via Cyrius CHANGELOG (sandhi repo is frozen). Sidebar: bundled `lib/sandhi.cyr` carries a stale "single-threaded until 0.8.0" comment from before the 0.8.0 scope shift — sandhi-side; not daimon's to fix.

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
