# Daimon Roadmap

> **Severity legend** (applied to open items on the current work arc):
> **P0** blocking — security / correctness regression, must-fix before next ship.
> **P1** high — must-have for the current arc to close.
> **P2** medium — should-have, schedule when capacity opens.
> **P3** / **Low** — nice-to-have, no urgency; no consumer pressure today.
>
> Upstream-blocker items quote the upstream tracker's severity directly (cyrius / sandhi / sakshi each use their own P-scale; daimon copies the upstream rating). Completed items don't carry severity (the work is done).

## Completed (v0.7.0)

- [x] Full Rust → Cyrius port (9,724 LOC → 4,141 LOC, 15 modules, 24 endpoints)
- [x] Test suite (200 assertions / 26 groups), benchmarks (16), fuzz harnesses (5)
- [x] Security audit + remediation (9/10 fixed, 1 accepted risk)
- [x] Modern Cyrius 4.2.0 toolchain, CI/CD pipelines

## Security Gates (trigger-based)

- [ ] **P0 (gated)** — **VULN-007: Bump allocator memory zeroing.** **MUST fix before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted federation, external MCP callbacks (bote). Remediation: per-agent arena allocators with zero-on-reset. Severity is **P0 when triggered**, dormant today because no consumer has flipped any of the gating conditions. Re-evaluate at every v1.x.0 cut.

## Blocked on Upstream Ports

- [ ] **Low** — Firewall MCP tools — [nein](https://github.com/MacCracken/nein) core port complete (v0.1.0, 13 modules); MCP tool wiring still pending nein's own `mcp` module which is blocked on bote. No active consumer demand; bumps to **P2** when a consumer asks for firewall control via daimon's MCP surface.
- [ ] **P2** — MCP tool hosting (bote re-exports) — blocked on [bote](https://github.com/MacCracken/bote) Cyrius port. Bote is actively shipping (2.7.2 as of 2026-05-11; the `dist/bote-core.cyr` opt-in transport-free profile landed in 2.7.2 and t-ron 2.1.3 is the trigger consumer); the re-exports become wirable as bote's libro_tools port lands. Tracked passively until bote signals the dispatch surface is stable.
- [x] ~~MCP tool call forwarding — blocked on bote + HTTP client library~~ — HTTP-client side unblocked by sandhi (`sandhi_rpc_mcp_call` available since Cyrius 5.7.0). Wired daimon-side at 1.2.1.

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

## Completed (v1.2.2)

- [x] **Lower sandhi `idle_ms` below the 30s default** — `serve` (sync) now uses `sandhi_server_run_opts` with `idle_ms = SERVE_IDLE_MS = 5000`. `serve_async` (async) applies `SO_RCVTIMEO = SERVE_IDLE_MS` per accepted cfd via the new `set_recv_timeout_ms` helper — closes a pre-existing slowloris gap in async (no per-connection timeout since 1.1.0). Both paths now bound the slow-sender hold at ~5 s instead of sandhi's 30 s default.
- [ ] **Collapse `serve_async` to `sandhi_server_run_opts`** — **still blocked upstream.** Bundled sandhi 1.3.3 still accepts-but-does-not-honor `sandhi_server_options_max_conns`. Tracked upstream at [sandhi/docs/issues/2026-05-10-daimon-server-max-conns.md](https://github.com/MacCracken/sandhi/blob/main/docs/issues/2026-05-10-daimon-server-max-conns.md) (severity **Low**); re-check at every cyrius pin bump.

## Completed (v1.2.4)

- [x] **Cyrius pin `5.10.44` → `6.1.24`** — first 6.x toolchain pin. Bundled sandhi rides up `1.3.3` → `1.4.10`; `cyrius.lock` re-resolved to 53 deps (was 37). No daimon source changes; 213/213 tests pass, 0 lint warnings.
- [x] **Sakshi pin `2.2.3` → `2.2.10`** — patch refresh; `msg_len`-required call surface unchanged since 2.0.0.
- [x] **`./lib/` snapshot refreshed** — stale 5.10.44 stdlib (sandhi 1.3.3) deleted; `cyrius deps` re-resolved against the 6.1.24 version-pinned snapshot.
- [x] Verified sandhi 1.4.10 keeps daimon's API surface (`sandhi_server_run{,_opts}`, `sandhi_server_options_idle_ms`, `sandhi_server_recv_request`, `sandhi_rpc_mcp_call`) and the `TLS_EARLY_DATA_ACCEPTED` rationale for the transitive tls/mmap/dynlib/fdlopen deps.
- [x] Build size noted: ~624 KB (1.2.2) → ~1.43 MB — 6.x DCE NOPs-but-keeps dead code vs. 5.x stripping it; toolchain effect, not a daimon regression.

## v1.2.x — Current work arc

Open items on the current arc, severity-tagged. The arc closes when the P2s land + the P3s drain (no hard cap; per the working-loop convention, ship when ready).

- [ ] **P2** — `guides/quickstart.md` refresh — install one-liner references the 5.7.12 / sakshi 2.0.0 era (versioned toolchain layout + `cyrius deps` workflow + `lib/` gitignored). Load-bearing for new-user onboarding; an incorrect install command actively breaks first-run.
- [ ] **P2** — `docs/architecture/overview.md` refresh — stdlib deps list adds tls/mmap/dynlib/fdlopen (1.2.0 transitive add via sandhi 1.3.3); sandhi 1.3.3 notes; `lib/` gitignored. Reference doc consulted on every architectural decision; staleness propagates downstream.
- [ ] **P3** — `README.md` footprint block — cyrius 6.1.24, sakshi 2.2.10, ~1.43 MB binary (6.x DCE keeps NOPed-but-unstripped dead code; see 1.2.4 CHANGELOG), refreshed dep list. Marketing-surface, not load-bearing for correctness.
- [ ] **P3** — `CONTRIBUTING.md` workflow steps — cyrius pin, `cyrius deps` workflow, lib/ gitignored expectation, fmt-via-diff gate + lint-fail-on-warn posture. Onboarding refinement; not blocking.
- [ ] **P3** — `BENCHMARKS.md` re-baseline under 5.10.34. Within-noise expected — no microbenchmark touches HTTP. Useful for the "prove the wins" discipline but no consumer pressure.
- [ ] **P3** — `guides/api.md` cyrius pin + example commands refresh.
- [ ] **P2 (unblocked at 1.2.4)** — **Collapse `serve_async` onto sandhi's `sandhi_server_run_async`** — upstream resolved as of bundled sandhi **1.4.9** (rides in with cyrius 6.1.24): `sandhi_server_run_async` is an epoll-cooperative accept loop that **does honor `sandhi_server_options_max_conns`** (the sync `sandhi_server_run` / `sandhi_server_run_opts` remain single-flight by design). Daimon's `serve_async` currently hand-rolls its own epoll accept loop (`set_recv_timeout_ms` + `async_spawn` per cfd, since 1.1.0). The collapse is now actionable — it's a behavioural change, so it earns a dedicated work-loop cycle with tests + benchmarks (concurrency-under-load comparison vs. the hand-rolled loop). Bumped from **Low (upstream-blocked)** → **P2 (actionable)** at the 1.2.4 pin bump.

**Upstream-blocker items** (not in daimon's hands; tracked for visibility):

- [ ] **P2 (upstream cyrius)** — aarch64 cross-build `SYS_EPOLL_WAIT` gap — `lib/async.cyr` references the constant unconditionally, but `lib/syscalls_aarch64_linux.cyr` doesn't define it (aarch64 has no plain `epoll_wait` syscall). Daimon CI tolerant via warn-on-detect; aarch64 binaries return automatically when upstream patches. Tracked at [cyrius/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md](https://github.com/MacCracken/cyrius/blob/main/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md) (pinned in cyrius roadmap under `v5.10.x — Held`).

## Future (v1.3.0+)

Severity assigned at v1.3.0 cut once the next arc's shape is chosen. Today these are unsequenced — none are blocking the v1.2.x close.

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
