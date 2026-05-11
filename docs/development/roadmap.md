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

## Completed (v1.2.0)

- [x] Cyrius pin `5.7.12` → `5.10.34` (15 patch slots: struct-by-value ABI completion, `lib/tls.cyr` early-data accessors, sandhi 1.3.3 fold-in, hashmap key-type variants, `#derive(accessors)` / `#derive(Serialize)`, `cyrius distlib` profile bundles).
- [x] Sakshi pin `2.0.0` → `2.2.3` (arch-portable syscalls, `sakshi_clock_recalibrate()`, 5.8.65 stdlib fold-in).
- [x] `/lib/` gitignored — `cyrius deps` repopulates from version-pinned snapshot.
- [x] CI/release workflows rewritten on the libro/bote/agnosys 5.10.x shape: versioned toolchain installer, source-archive `lib/` fetch, `cc5_aarch64` top-level pickup, workflow env (`CYRIUS_DCE`, `CYRIUS_NO_WARN_SHADOW_LIB`), `cc5 --version` verify, fmt re-enabled via diff, lint flipped fail-on-warn.
- [x] `docs/doc-health.md` ledger added (agnosys / cyrius convention).
- [x] Build clean at 622 KB DCE, 200/200 tests pass, 0 lint warnings.

## Completed (v1.2.1)

- [x] **External MCP forwarding via `sandhi_rpc_mcp_call`** — `api_mcp_call` dispatch path now forwards to the external endpoint, maps transport / JSON-RPC / success outcomes to 502 / 200-isError / 200-passthrough respectively. `api_mcp_register` enforces `validate_callback_url` (SSRF guard). The original 1.1.5 plan to add `McpToolDescription.endpoint_url` was rescoped — the existing external-wrapper struct (`{tool, callback_url}`) already separates URL from tool description, so `mcp_find_external_url` is the new accessor instead. End-to-end roundtrip test against a fake MCP server deferred (needs a localhost fixture not yet present in the test tree).

## v1.2.2 — Sandhi idle_ms + serve_async collapse (carryover from v1.1.5)

- [ ] **Lower sandhi `idle_ms` below the 30s default** — `serve` calls `sandhi_server_run` with `opts = 0` so the slowloris timeout sits at sandhi's 30 000 ms default. Collect a baseline of legitimate-client P99 request durations from 1.2.0 / 1.2.1 production soak data, then thread `sandhi_server_options_new()` + `sandhi_server_options_idle_ms(opts, N)` + `sandhi_server_run_opts(...)` with N ≈ 5 000 ms. Trade-off documented in `docs/audit/2026-04-27-sandhi-migration.md` § VULN-008.
- [ ] **Collapse `serve_async` to `sandhi_server_run_opts`** — premise-check first against sandhi 1.3.3 (bundled in cyrius 5.10.34): if `sandhi_server_options_max_conns` enforcement landed, daimon's `serve_async` (epoll loop + per-call buf alloc + inline smuggling-check duplication) collapses into one `sandhi_server_run_opts(...)` call shared with sync. If not yet enforced, keep tracking via Cyrius CHANGELOG.

## v1.2.x — Doc cleanup (rolling)

- [ ] README footprint block (cyrius 5.10.34, sakshi 2.2.3, ~622 KB binary, stdlib dep list)
- [ ] CONTRIBUTING workflow steps + cyrius pin + `cyrius deps` usage + lib/ gitignored note
- [ ] architecture/overview.md stdlib deps + sandhi 1.3.3 notes
- [ ] BENCHMARKS re-baseline under 5.10.34 (within-noise expected — no microbenchmark touches HTTP)
- [ ] guides/quickstart.md cyrius install one-liner (versioned layout)
- [ ] guides/api.md cyrius pin + example commands

## Future (v1.3.0+)
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
