# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [1.2.2] - 2026-05-10

**Slowloris bound, on both sync and async.** Lowers the per-connection idle timeout from sandhi's 30 000 ms default to **5 000 ms** on the sync path (via `sandhi_server_run_opts` + `sandhi_server_options_idle_ms`), and **closes a pre-existing async-path slowloris gap** by applying `SO_RCVTIMEO` to async-accepted cfds (async had no per-connection timeout since 1.1.0). Second of the two 1.1.5 sandhi follow-ups; the third (`serve_async` collapse into `sandhi_server_run_opts`) stays deferred — upstream sandhi's `max_conns` is still accepted-but-not-honored, see Known issues.

### Added

- **`SERVE_IDLE_MS = 5000`** — centralized idle-timeout constant in `src/main.cyr`. Used by both `serve` (sync, via sandhi opts) and `serve_async` (async, via direct `SO_RCVTIMEO` syscall). One source of truth; keeps both paths in lock-step.
- **`set_recv_timeout_ms(fd, ms)`** — daimon-side helper that applies `SO_RCVTIMEO` via `syscall(SYS_SETSOCKOPT, fd, SOL_SOCKET=1, SO_RCVTIMEO=20, &tv, 16)`. `tv` is a 16-byte `struct timeval` `{tv_sec, tv_usec}`. Mirrors what `sandhi_server_run_opts` does internally for the sync path. Daimon-side rather than calling the underscore-prefixed `_sandhi_conn_set_timeout_ms` because that's a private sandhi internal.

### Changed

- **`serve` (sync)** — replaces `sandhi_server_run(addr, port, &handle_request, 0)` with `sandhi_server_run_opts(addr, port, &handle_request, 0, opts)` where `opts = sandhi_server_options_new()` + `sandhi_server_options_idle_ms(opts, SERVE_IDLE_MS)`. sandhi's `sandhi_server_run_opts` accept loop applies `SO_RCVTIMEO = SERVE_IDLE_MS` per accepted connection (lib/sandhi.cyr:11629-11631) — same enforcement mechanism the previous default went through, just with a tighter bound.
- **`serve_async`** — calls `set_recv_timeout_ms(cfd, SERVE_IDLE_MS)` immediately after a successful `syscall(SYS_ACCEPT, sfd, 0, 0)`, before `async_spawn`. Async-handler structure unchanged.

### Security

- **VULN-async-slowloris (newly classified) — closed at 1.2.2.** Pre-1.2.2, daimon's async path had **no** per-connection timeout: `async_await_readable(cfd)` in `async_handle_client` would block indefinitely on a slow sender, and `sandhi_server_recv_request` ran without an `SO_RCVTIMEO` set on the fd. The 1.1.4 sandhi-migration audit noted the sync-path bound ("the worst case is now a 30 s `SO_RCVTIMEO` per malicious connection") but did not surface the async-path asymmetry. 1.2.2 closes this — async accepted fds now carry the same 5 000 ms `SO_RCVTIMEO` the sync path does.
- **VULN-001 / VULN-008 trade-off (carried forward from 1.1.4 audit) — improved.** The 1.1.4 ship documented the worst case as a 30 000 ms hold per malicious connection (sandhi's default `SO_RCVTIMEO`). 1.2.2 lowers that to 5 000 ms uniformly. Sandhi's `HSV_REQ_BUF_SIZE = 65 536` memory cap remains the orthogonal bound. No regression in legitimate-client behaviour expected — daimon's request handler is in-memory (no network egress in the happy path), so legitimate P99 sits well under 1 s.

### Verified

- `cyrius check --with-deps`: ok.
- `cyrius build` (DCE): **624 KB** statically-linked ELF (was 623 KB at 1.2.1; +352 bytes from the helper + opts wiring).
- `cyrius test`: **213 / 213** assertions pass (no test additions — both wirings exercise sandhi/syscall paths that aren't reachable from unit tests; integration verification is via running the binary against curl with slow-sender simulation in 1.2.x doc cleanup).
- `cyrius lint`: 0 warnings across src/ + tests/.
- `cyrius fmt`: stable.
- aarch64 cross-build: still blocked on the upstream `SYS_EPOLL_WAIT` gap (tracked upstream at [cyrius/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md](https://github.com/MacCracken/cyrius/blob/main/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md), severity **P2**); CI warn-on-detect path triggers cleanly.

### Known issues

- **`serve_async` collapse into `sandhi_server_run_opts` — still deferred upstream.** Bundled sandhi 1.3.3 (at cyrius 5.10.34) accepts `sandhi_server_options_max_conns(opts, n)` but does not honor it — the accept loop in `sandhi_server_run_opts` remains single-flight regardless of the configured value. Full write-up + auto-resolve mechanism tracked upstream at [sandhi/docs/issues/2026-05-10-daimon-server-max-conns.md](https://github.com/MacCracken/sandhi/blob/main/docs/issues/2026-05-10-daimon-server-max-conns.md) (severity **Low**). Re-checked at every cyrius pin bump; collapses to a small follow-up patch when upstream wires the enforcement (worker pool or epoll-cooperative).

## [1.2.1] - 2026-05-10

**External MCP forwarding lights up.** Replaces the `api_mcp_call` `"tool dispatch not available in sync mode"` stub at `src/main.cyr:3393` with a real `sandhi_rpc_mcp_call` dispatch path. Carryover from the 1.1.5 roadmap, sequenced as the first behavioural change of the 1.2.x arc.

### Added

- **`mcp_find_external_url(reg, name_cstr)`** — sibling lookup to `mcp_find_tool`. Returns the callback URL registered for an external tool, or `0` for builtin / not-found. Lets the dispatch site decide builtin vs external routing without re-parsing the wrapper.
- **HTTP 502 ("Bad Gateway")** added to `_http_reason` — emitted by `api_mcp_call` on upstream transport failure.

### Changed

- **`api_mcp_register`** — now requires a `callback_url` field and validates it via `validate_callback_url` (SSRF guard: http:// or https:// only). Missing/empty/non-http(s) URLs return 400 with a specific message. Pre-1.2.1 silently registered tools with empty URLs (which would then be impossible to dispatch); rejecting at the boundary surfaces the error at registration time.
- **`api_mcp_call`** — full implementation, replaces the sync-mode stub:
    - Tool not found → 400 (unchanged).
    - Tool found but builtin (no callback URL) → **501 Not Implemented** with `"builtin tool dispatch not implemented"`. Distinct from "not found" — the route exists but daimon has no in-process dispatcher for builtin tools today.
    - Tool found + external → forward to `sandhi_rpc_mcp_call(url, "tools/call", body)`. The inbound `/v1/mcp/call` body shape (`{name, arguments}`) is already the MCP `tools/call` params shape, so the body is passed through verbatim as JSON-RPC params. Sandhi wraps with `{"jsonrpc":"2.0","id":N,"method":"tools/call","params":…}`.
    - Transport failure (connect / TLS / timeout / non-2xx HTTP) → **502 Bad Gateway** with the sandhi error message embedded as JSON-escaped `upstream`.
    - JSON-RPC error envelope (`error.code != 0`) → **200 OK** with the MCP `{"content":[{"type":"text/plain","text":"…"}],"isError":true,"code":N}` shape, so MCP clients see a normal MCP error rather than a transport error.
    - Success → **200 OK** with the upstream `result` value passed through. Empty result (neither error nor result envelope) returns `{"content":[],"isError":false}`.

### Security

- **VULN-mcp-register-url (new) — closed.** `api_mcp_register` previously accepted any string (or no string at all) as `callback_url`. With `sandhi_rpc_mcp_call` now wired, an attacker who could register a tool could have caused daimon to fetch arbitrary URL schemes (`file://`, `gopher://`, internal-network HTTP — classic SSRF surface). The `validate_callback_url` allow-list (http:// + https:// only) is now enforced at the boundary. The validator was always present in `src/main.cyr:1323`; 1.2.1 wires it in.
- **No new HTTP attack surface from the dispatch path** — daimon never echoes the inbound body to the upstream; it parses the JSON for routing and hands the original cstr to sandhi as params, which sandhi wraps in a JSON-RPC envelope. JSON injection on the upstream side requires already-tampered tool registration, which now goes through the URL validator.

### Tests

- **+13 assertions** (200 → **213**). New coverage in `tests/daimon.tcyr` mcp_registry group:
    - External tool registration + name lookup + URL roundtrip.
    - `mcp_find_external_url` returns 0 for builtin and missing tools.
    - `validate_callback_url` boundary cases: null, empty, `file://`, `ftp://`, `javascript:` rejected; `http://` + `https://` accepted.

### Verified

- `cyrius check --with-deps`: ok.
- `cyrius build` (DCE): **623 KB** statically-linked ELF (1.2.0 was 622 KB; +1 360 bytes from the new dispatch path + 502 reason phrase).
- `cyrius test`: **213 / 213** assertions pass.
- `cyrius bench`: 16 microbenchmarks within noise of 1.2.0 (no benchmark touches the HTTP / MCP path).
- `cyrius lint`: 0 warnings across src/ + tests/.
- `cyrius fmt`: stable.

### Deferred (still in v1.2.x)

- **End-to-end roundtrip test** against a fake MCP server — requires a localhost listener fixture that's out of scope for tcyr unit tests. Will land alongside the v1.2.2 sandhi tuning work or as a v1.2.x rolling addition once the fixture pattern is settled.
- **Builtin tool dispatcher** — `api_mcp_call` returns 501 for builtin tools today. No in-tree consumer registers builtins through the HTTP path; if one shows up, builtin dispatch lands as its own slot.

## [1.2.0] - 2026-05-10

**Toolchain modernization + CI/release rewrite.** Bumps Cyrius 5.7.12 → 5.10.34 and sakshi 2.0.0 → 2.2.3, gitignores `/lib/`, and rebuilds CI/release on the libro/bote/agnosys 5.10.x shape. No public-API changes; same 24 endpoints, same wire shape. Internal tightening from 15 patch slots of Cyrius improvements between 5.7.12 and 5.10.34.

### Changed

- **Cyrius pin**: `5.7.12` → `5.10.34`. Covers 15 patch slots of upstream improvements (struct-by-value ABI completion, `lib/tls.cyr` early-data accessors, sandhi 1.3.3 fold-in, hashmap key-type variants, `cyrius distlib` profile bundles, `#derive(accessors)` + `#derive(Serialize)` on structs). Daimon's source compiles unchanged under the new pin — the 4 long-line lint warnings + 1 false-positive `unclosed braces` warning from 5.7.12 are all gone (no source change required).
- **Sakshi pin**: `2.0.0` → `2.2.3`. Picks up arch-portable syscalls (x86_64 + aarch64 dispatched at compile time via `_SK_SYS_*`), `sakshi_clock_recalibrate()` for long-running processes, and the 5.8.65 stdlib fold-in patch. No daimon source change — the `msg_len`-required call surface from 2.0.0 is unchanged.
- **`cyrius.cyml` stdlib deps**: added `tls`, `mmap`, `dynlib`, `fdlopen`. Required at compile time because sandhi 1.3.3's bundle unconditionally references `TLS_EARLY_DATA_ACCEPTED` for its TLS 1.3 0-RTT client-write path (`sandhi_rpc_*`). Daimon doesn't use sandhi's HTTP client today — server only — so the runtime cost is zero (DCE drops the unused paths); the deps just have to be on disk for the bundle to compile.
- **`/lib/` is now gitignored**. Repopulated by `cyrius deps` from the version-pinned stdlib snapshot + the `[deps.sakshi]` git pin. Matches the libro / bote / agnosys / patra / yukti convention. Removes 35 vendored stdlib files from the repo (binary `git diff` size shrinks substantially).
- **`src/main.cyr`** — formatter-applied: 9 continuation-line indentation fixes (4 → 8 spaces) on lines 1034-1038, 2825-2827, 3057. Cosmetic; no behavioural change. `cyrius fmt` is stable on 5.10.34 (the 5.7.12 truncation-at-line-4168 bug is fixed upstream).
- **`tests/daimon.tcyr`** — 3 long-line lint warnings cleared: dedup `else` branch split into `elif`; rag-pipeline ingest string extracted to a local; merge dup-result comment moved above the `vec_push` instead of trailing it.

### CI / Release

CI and release workflows rewritten on the **libro / bote / agnosys 5.10.x shape**:

- **Toolchain installer** — versioned layout (`~/.cyrius/versions/<V>/{bin,lib}/` + symlinks + `~/.cyrius/current`). Required by cc5 5.10.9+ which resolves arch-peer includes (`syscalls_x86_64_linux.cyr`, etc.) through this path. Pulls both the release tarball (binaries + first-party deps cache) **and** the GitHub source archive at the version tag (for the `lib/` stdlib snapshot — 5.10.x release tarballs ship `bin/` + `deps/` only).
- **`cc5_aarch64` top-level pickup** — moved out of `bin/` to the tarball top level at Cyrius 5.7.48; explicit copy step picks it up so the aarch64 cross-build keeps working.
- **Workflow env** — `CYRIUS_NO_WARN_SHADOW_LIB=1` silences the post-5.10.x `./lib/ shadows version-pinned ...` informational note across all steps; `CYRIUS_DCE=1` set at workflow level (was per-step).
- **`cc5 --version` verify** — added between install and dep-resolve.
- **`cyrius fmt` re-enabled** — `diff -q <(cyrius fmt $f) $f` per file (5.9.x+ `--check` is a no-op). Covers src/ + tests/ + bench/ + fuzz/. Daimon is clean on first run; gate fires on drift.
- **Lint flipped fail-on-warn** — `continue-on-error: true` removed. Daimon is clean under 5.10.34 (was 6 standing warnings on 5.7.12, all resolved upstream or by the cosmetic edits above).
- **Docs job** — adds `docs/doc-health.md` to the required-files list.
- **Release workflow** — mirrors agnosys release.yml: split into `ci` → `build` → `release` jobs; ships `daimon-<tag>-src.tar.gz` + `daimon-<tag>-x86_64-linux` + `daimon-<tag>-aarch64-linux` (when `cc5_aarch64` is present **and** the cross-build clears the upstream stdlib aarch64 gap — see Known issues below) + `cyrius.lock` + `SHA256SUMS`; pre-release detection on both `0.x` and `v0.x` tag styles.
- **aarch64 cross-build is tolerant of upstream stdlib gaps** — known-blocker symbols (currently `SYS_EPOLL_WAIT`) downgrade to a `::warning::` and exit 0. Any other failure still fails the step. Same posture as sakshi 2.2.2's aarch64 lane. Tracked upstream at [cyrius/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md](https://github.com/MacCracken/cyrius/blob/main/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md) (severity **P2**).

### Known issues

- **aarch64 cross-build blocked on upstream stdlib gap.** `lib/async.cyr` references `SYS_EPOLL_WAIT` unconditionally, but `lib/syscalls_aarch64_linux.cyr` only defines `SYS_EPOLL_PWAIT` (aarch64 has no plain `epoll_wait` syscall). Reproduces on both cyrius 5.10.34 and 5.10.47. Daimon's source is portable — the gap is in the cyrius stdlib. CI / release downgrade this specific error to a warning so the x86_64 ship is unblocked; aarch64 binaries return automatically when upstream patches `lib/async.cyr` or adds an arch-dispatch shim. Full write-up + workaround mechanism tracked upstream at [cyrius/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md](https://github.com/MacCracken/cyrius/blob/main/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md) (severity **P2**).

### Added

- **`docs/doc-health.md`** — living ledger of doc currency (fresh / stale / read-through / evergreen / archive / open-question buckets per tier). Pattern lifted from agnosys / cyrius. Refresh discipline documented at the foot.

### Verified

- `cyrius check --with-deps src/main.cyr`: ok (one expected `shadow lib/` note silenced by `CYRIUS_NO_WARN_SHADOW_LIB=1`).
- `cyrius build src/main.cyr build/daimon` (DCE on): **622 KB** statically-linked ELF. Size delta vs 1.1.4 (452 KB) is the tls/mmap/dynlib/fdlopen modules dragged in by sandhi 1.3.3's unconditional 0-RTT constant refs — DCE drops most of the body, the headers + symbol tables account for the +170 KB.
- `cyrius test tests/daimon.tcyr`: **200 / 200** assertions pass across 26 test groups. No test changes required for the toolchain bump.
- `cyrius lint`: 0 warnings across `src/`, `tests/`, `bench/`. Was 6 warnings on 5.7.12.
- `cyrius fmt`: stable across `src/`, `tests/`, `bench/` (`diff` returns no drift).

### Roadmap

- 1.1.5 sandhi follow-ups rescoped to 1.2.1 / 1.2.2 — see `docs/development/roadmap.md`.
- "Future (v1.2.0+)" items (jnana / gRPC / WebSocket / distributed tracing / agent migration) renamed to "Future (v1.3.0+)".

## [1.1.4] - 2026-04-27

### Changed

- **HTTP server migrated to `lib/sandhi.cyr`** (Cyrius 5.7.12 stdlib). The pre-1.1.4 hand-rolled HTTP layer in `src/main.cyr` (≈580 LOC of `http_*` parse + send fns + the body-recv loop in `handle_request`) is replaced. Endpoint handlers (`api_*`) are unchanged — daimon-named shims (`http_send_response`, `http_parse_method`, `http_parse_path`, `http_parse_content_length`, `http_has_transfer_encoding`, `http_parse_query_param`, `http_parse_body`) preserve the call surface and delegate to `sandhi_server_*`.
- **Sync server (`serve`)** now delegates to `sandhi_server_run(INADDR_ANY(), port, &handle_request, 0)` — sandhi owns bind / listen / accept / recv / smuggling rejection (CL+TE conflict per RFC 7230 §3.3.3, Host.Host / CL.CL / TE.TE duplicates per §3.3.2 + §5.4) and closes the connection after the handler returns.
- **Async server (`serve_async`)** keeps its epoll-cooperative accept loop but uses `sandhi_server_recv_request`, `sandhi_server_request_has_cl_te_conflict`, and `sandhi_server_request_has_dup_smuggling_header` inline before dispatching to the same shared `handle_request`. Fresh per-call buffer (sandhi's process-global `_hsv_req_buf` is safe under the sync single-threaded loop but the async path explicitly allocates per call to keep the no-interleave invariant explicit).
- **`handle_request` is now sandhi-shape**: `(ctx, cfd, buf, blen)`. Sync caller is `sandhi_server_run`; async caller is `async_handle_client`. Same code runs under both modes.
- **CLI banner fixed** (pre-existing bug surfaced by the migration's smoke test): six `sakshi_info` / `sakshi_warn` / `sakshi_error` call sites in `src/main.cyr` were missing the required `msg_len` argument since the sakshi 2.0.0 stdlib bump. The startup banner emitted random buffer contents under sakshi as a result. Fixed by passing explicit byte lengths; the banner now reads `daimon vX.Y.Z listening on port N (mode)` cleanly.
- **`lib/http.cyr` dep dropped** from `cyrius.cyml` `[deps] stdlib`. Daimon never used the HTTP client; sandhi covers any future need.
- **CLAUDE.md** sandhi note updated from "recommended for new HTTP server work" to "in use".

### Security

Re-audited all 10 VULN findings from `docs/audit/2026-04-13-security-audit.md` against the new code path. Full report at `docs/audit/2026-04-27-sandhi-migration.md`.

- **VULN-001 (request smuggling): strengthened.** Two layers cover this end-to-end: sandhi's accept-loop rejection (CL+TE conflict, duplicate Host / CL / TE per RFC 7230) before the handler, plus daimon's continued `Transfer-Encoding` rejection inside the handler. Sandhi's `sandhi_server_content_length` is RFC 7230 §3.3.2 strict (rejects any non-digit in the value), closing the loose-digit CL.CL sub-vector that the old `http_parse_content_length` accepted (`"10, 20"` parsed as 10).
- **VULN-008 (oversized request DoS): bounded with a different latency profile.** Sandhi caps the buffer at `HSV_REQ_BUF_SIZE = 65 536` regardless of declared `Content-Length` — memory bound is preserved. The pre-1.1.4 fast-413 (parse CL on first recv, reject before reading body) is no longer reachable; the worst case is now a 30 s `SO_RCVTIMEO` per malicious connection. Net trade-off is favourable: the old path had **no** SO_RCVTIMEO, so a slowloris attacker could tie up a worker indefinitely under 1.1.3.
- **VULN-009 (per-IP rate limiting): unchanged.** `rate_check(cfd)` still runs at the top of `handle_request`, before any sandhi-side parsing.
- **VULN-002, VULN-004, VULN-005, VULN-006, VULN-010: unchanged** — none touch the HTTP path.

### Observed

- Binary size **263 KB → 452 KB** (DCE on). The reachable subset of sandhi (server fns, request parsing, header checks, smuggling rejection) is what shipped; HTTP/2, SSE, TLS, and JSON-RPC modules ride along but don't bloat (no dead-fn warnings against them in `cyrius build` output).
- 200 / 200 unit tests pass. End-to-end smoke (`/v1/health`, `/v1/agents` GET + POST, `/v1/missing`, `/v1/edge/nodes?status=online`, `/v1/metrics`) passes against both `serve` and `serve --async`. Smuggling tests via raw socket: dup-Host → 400, CL.CL with full body → 400, TE-only → 501.
- Benchmarks (16 internal microbenchmarks) within noise of 1.1.3 — none of them exercise the HTTP server, so the migration shouldn't move them and didn't.

### Deferred (tracked in `docs/development/roadmap.md` § v1.1.5)

- **External MCP forwarding** via `sandhi_rpc_mcp_call` — replaces the `"tool dispatch not available in sync mode"` stub. Needs `McpToolDescription.endpoint_url`.
- **Lower sandhi `idle_ms`** below the 30 s default once a 1.1.4 production soak surfaces a baseline P99.
- **`serve_async` collapse to `sandhi_server_run_opts`** once a Cyrius stdlib patch wires up `sandhi_server_options_max_conns` enforcement. The hook is already public — sandhi 1.0.0 (folded into Cyrius 5.7.0 stdlib, sandhi repo now in maintenance mode) deliberately landed HTTP/2 + client connection pool at 0.8.0 in favour of nailing single-server hardening first; the 0.9.x P0/P1 sweep validated that call. Wire-up is straightforward when scheduled.

## [1.1.3] - 2026-04-27

### Added

- **`[release]` table** in `cyrius.cyml` — declares `bins = ["daimon"]` + `cross_bins = ["daimon-aarch64"]` as the canonical distribution list. `release.yml` continues to drive GitHub releases directly, but future tooling (`cyrius package`, ark) reads from this single source instead of duplicating the binary list.

### Changed

- **Lint warnings reduced 6 → 2** in `src/main.cyr`. Fixed long lines:
    - `rag_config_default` prompt template extracted to a `prompt_tmpl` local (was a 119-char `str_from(...)` inline at L1075).
    - `agent_ipc_accept_one` big-endian u32 length parse wrapped across 4 lines (was a single 119-char chained `load8 * 16777216 + ...` at L2823).
    - `api_mcp_call` inline JSON literal split into 3 `str_builder_add_cstr` calls (was a single 122-char literal at L3393).
    - `ip_to_cstr` dotted-decimal print block reformatted with one statement per line (the `if (val >= 100) {...}` and `elif (val >= 10) {...}` were single 120/123-char lines at L3918-3919).
- Remaining 2 warnings are confirmed cyrius 5.7.12 linter false positives — see "Known issues" below. CI lint step stays `continue-on-error: true` until upstream fixes land.

### Known issues

- **`cyrius lint` false positive: `unclosed braces at end of file`**. The linter doesn't track string-literal state, so daimon's str-builder JSON pattern (`"{\"id\":\""` openings paired with `"}"` closings across separate calls) registers as 30 `{` vs 37 `}` overall and triggers the EOF brace warning. File parses cleanly (`cyrius check --with-deps` ok; 200/200 tests pass).
- **`cyrius lint` false positive: `trailing whitespace` at line 4177**. Comment line `# Parse remaining args: port number or --async` has zero trailing whitespace under `cat -A`, `git diff --check`, and `owl -A`. Persists after editing the comment text. Most likely a linter line-counter bug after multi-line replacements earlier in the file. Reproduces on cyrius 5.7.12.
- **`cyrius fmt` truncates `src/main.cyr`** mid-string at line 4168 on cyrius 5.7.12. Discovered when piping `cyrius fmt | cyim --write src/main.cyr` produced a 4134-line truncated file (was 4210). Hence `cyrius fmt --check` is not gated in CI; reverted via `git checkout` and unaffected since.

### Deferred (Tier 2 — separate release)

- `lib/sandhi.cyr` adoption to replace daimon's hand-rolled HTTP server (`http_parse_method`, `http_send_response`, etc., L2979-3556). Sandhi brings HTTP/2, SSE, JSON-RPC + MCP-over-HTTP (`sandhi_rpc_mcp_call`) — would also unstub daimon's external MCP forwarding in sync mode (currently returns the "tool dispatch not available in sync mode" error reformatted in this patch).
- `lib/atomic.cyr` for circuit-breaker counters in `cb_*` (L477-548): reviewed and explicitly skipped — both sync and async (epoll-cooperative) HTTP modes are single-threaded, so there is no concurrent reader/writer to atomicize. Adding atomics would solve a non-problem.
- `lib/sankoch.cyr` for memory-store compaction: deferred to its own release with a benchmark cycle.

## [1.1.2] - 2026-04-27

### Changed

- **Cyrius toolchain bump**: `4.5.0` → `5.7.12`. Build remains clean (`cyrius check --with-deps`), test suite holds (200 passed / 0 failed), binary builds (`build/daimon`, ~263 KB statically linked).
- **Manifest migration**: `cyrius.toml` → `cyrius.cyml` (5.0.0 breaking; `cyrius update` semantics). `version` now resolves from the `VERSION` file via `${file:VERSION}`. `output` moved under `build/` to match the modern layout used by vidya 2.3.0.
- **`sakshi` is no longer stdlib**: dropped from `[deps] stdlib`, added as `[deps.sakshi]` git pin (`tag = "2.0.0"`). After `cyrius deps`, `lib/sakshi.cyr` is a symlink into `~/.cyrius/deps/sakshi/2.0.0/dist/sakshi.cyr`.
- **`math` added to stdlib deps**: `f64_sqrt` (used by `vector_normalize` and `cosine_similarity`) needed `lib/math.cyr` registered explicitly so `cyrius vet` resolves it; build was implicitly pulling it in via stdlib auto-prepend, vet now reports `2 deps, 0 untrusted, 0 missing`.
- `cyrius.lock` now committed (5.7.8 made lockfile-on-by-default).
- CLAUDE.md: updated Cyrius pin reference (4.2.0 → 5.7.12), added note that `lib/sandhi.cyr` is the recommended HTTP path for new server work, documented `hashmap` key-type variants and external sakshi dep.

### CI / Release

- **CI overhauled** (`.github/workflows/ci.yml`) modeled on the modern agnostik 5.7.x shape: toolchain version pulled from `cyrius.cyml` (was `.cyrius-toolchain`); added `cyrius deps --verify`, `cyrius vet`, DCE build (`CYRIUS_DCE=1`), ELF magic check, best-effort aarch64 cross-build, per-test loop with discrete failure visibility, `cyrius bench` step, security-scan job (no `sys_system`, no writes to `/etc|/bin|/sbin`, no ≥64 KB stack buffers — comment-aware), and a docs job that checks 12 required files + version-in-CHANGELOG consistency.
- **`cyrius lint` runs as advisory** (`continue-on-error: true`) — daimon has 6 standing warnings (4 long lines + 1 false-positive brace from JSON-in-string-literal at line 3393). Tier-3 cleanup will eliminate them and flip lint to fail-on-warn.
- **`cyrius fmt --check` not gated**: cyrius 5.7.12 fmt has a truncation bug on daimon's `src/main.cyr` (cuts mid-string at line 4168). Skipping the gate until the upstream fix lands.
- **Release workflow rewritten** (`.github/workflows/release.yml`) to mirror agnostik: accepts both `v1.2.3` and `1.2.3` tag styles, semver-shape verification, source tarball + cross-arch binaries + `SHA256SUMS` archive, per-version changelog extraction via awk, pre-release detection for `0.x` tags.
- **`scripts/version-bump.sh`** simplified — only writes `VERSION` now, since `cyrius.cyml` resolves `[package].version` from it via `${file:VERSION}`.
- **README.md / CONTRIBUTING.md** updated for `cyrius.cyml` + new pin.

## [1.1.1] - 2026-04-13

### Changed

- Roadmap: unblocked nein-core firewall work. [nein](https://github.com/MacCracken/nein) v0.1.0 Cyrius port shipped 13 modules (rule/table/chain/set/nat/bridge/engine/mesh/geoip/policy/builder/firewall/validate) — daimon can now depend on nein directly. Nein's own `mcp` module stays gated on bote.

## [1.1.0] - 2026-04-13

### Added

- **Async HTTP server** — `serve --async` flag enables epoll-based cooperative concurrency via `lib/async.cyr`. Handles multiple connections per accept cycle with batched async_run. Both sync and async modes share the same request handler and security controls.
- Documentation: architecture overview, API guide with all 24 endpoints, quickstart guide, 3 ADRs (port rationale, HTTP mode, security process).
- Vendored `lib/async.cyr` (epoll cooperative runtime), `lib/http.cyr` (HTTP client), `lib/thread.cyr`, `lib/callback.cyr` via updated `cyrius.toml` deps.

### Changed

- Removed `rust-old/` directory (16 GB, 9,724 LOC Rust source + build cache). Rust history available in git pre-v0.7.0 tags.
- Refactored serve loop: request handling extracted to `handle_request(cfd)`, server setup to `server_bind(port)` — shared by both sync and async modes.
- SECURITY.md updated for v1.x supported versions, Cyrius-specific scope.
- CLI: `serve [port] [--async]` — async mode optional, sync remains default.

## [0.7.0] - 2026-04-13

Complete rewrite from Rust to Cyrius. 9,724 LOC Rust → 4,141 LOC Cyrius. Binary: 181 KB (was 4.0 MB). Zero external dependencies.

### Added

- **15 modules ported** with full API parity: error, config, agent, supervisor, memory, vector_store, rag, mcp, screen, scheduler, federation, edge, ipc, api, logging.
- **24 HTTP API endpoints** — synchronous TCP server on port 8090:
  - `/v1/health` — service health
  - `/v1/agents` (GET/POST), `/v1/agents/{id}` (GET) — agent lifecycle
  - `/v1/mcp/tools` (GET/POST), `/v1/mcp/tools/{name}` (DELETE), `/v1/mcp/call` (POST) — MCP tool dispatch
  - `/v1/rag/ingest` (POST), `/v1/rag/query` (POST) — RAG pipeline
  - `/v1/edge/nodes` (GET/POST), `/v1/edge/nodes/{id}` (GET), `…/heartbeat` (POST), `…/decommission` (POST), `/v1/edge/stats` — edge fleet
  - `/v1/scheduler/tasks` (GET/POST), `…/{id}` (GET), `…/{id}/cancel` (POST), `/v1/scheduler/nodes` (POST), `/v1/scheduler/schedule` (POST), `/v1/scheduler/stats` — task scheduling
  - `/v1/metrics` — aggregate metrics
- **Scheduler**: NodeCapacity with resource fitting + bin-packing, schedule_pending with assignment decisions, preempt_check, tasks_for_node, CronScheduler with interval-based entries + validation, stats aggregation.
- **Federation**: cluster management, heartbeat health tracking (online/suspect/dead), Raft-like election (start_election, receive_vote_request, receive_vote, become_coordinator, step_down), 4-factor weighted node scoring (resource/locality/load/affinity), agent placement, cluster stats.
- **Edge fleet**: register with validation (empty name, duplicate, fleet-full), heartbeat, health check (suspect/offline thresholds), decommission, list with status filter, stats.
- **FederatedVectorStore**: collection/replica management, cross-node search merge with dedup + re-ranking, remove_node, stats.
- **IPC**: Unix domain socket AgentIpc (bind/accept/send, length-prefixed wire protocol, ACK/NACK, connection limits), message bus (named routing + broadcast + direct send), RPC registry.
- **Memory store**: CRUD with atomic write (tmp+rename), list_keys, list_by_tag, clear, usage_bytes, key validation + sanitization.
- **RAG pipeline**: ingest_text (chunk + embed + index), query_text (embed + search + format context).
- **Vector store**: cosine similarity, brute-force search with ranking, normalize_vec.
- **Agent lifecycle**: start/stop/pause/resume with race-free pidfd signal delivery, /proc resource monitoring (VmRSS, CPU time, fd count, thread count), resource limits on spawned processes.
- **Supervisor**: circuit breaker (Closed→Open→HalfOpen), output capture ring buffer, resource quotas, health tracking.
- **MCP**: tool registry (builtin + external) with manifest, register, deregister, validate_callback_url.
- **Screen capture**: permission manager with rate limiting, recording sessions (active/paused/stopped).
- CLI: `serve [port]`, `version`, `help`.
- Test suite: 200 assertions / 26 test groups.
- Benchmark suite: 16 benchmarks with Rust comparison (BENCHMARKS.md).
- Fuzz harnesses: 5 (circuit_breaker, memory_keys, scheduler_fsm, vector_store, mcp_registry).
- Security audit: docs/audit/2026-04-13-security-audit.md — 10 findings, 9 fixed, 1 accepted risk.

### Security

- **VULN-001**: Content-Length validation, Transfer-Encoding rejection (501), 413 Payload Too Large. Prevents request smuggling.
- **VULN-002**: `json_escape_str()` on all user-controlled strings in JSON responses. Prevents JSON injection.
- **VULN-004**: `pidfd_open()`/`pidfd_send_signal()` with `kill()` fallback. Prevents PID reuse race.
- **VULN-005**: Agent memory directories 0700 (was 0755).
- **VULN-006**: `SO_PEERCRED` UID verification on Unix socket accept. Prevents unauthorized IPC.
- **VULN-008**: `MAX_REQUEST_SIZE=65536`, Content-Length body reads. Prevents oversized request DoS.
- **VULN-009**: Per-IP rate limiting — 120 req/min sliding window, 429 Too Many Requests.
- **VULN-010**: `agent_spawn_with_limits()` with `RLIMIT_AS` + `RLIMIT_CPU`. Prevents agent resource exhaustion.
- HTTP query parameter bounds checking. Prevents buffer over-read.
- Empty path segment handling (`/v1/agents/` → 404).

### Changed

- **Language**: Rust → Cyrius. Rust source removed in v1.0.1.
- **Toolchain**: Cyrius 4.2.0 (pinned in `.cyrius-toolchain`).
- **Build**: `cargo build` → `cyrius build src/main.cyr build/daimon`.
- **HTTP**: Async (tokio/axum) → synchronous (raw TCP sockets).
- **Dependencies**: 193 crate dependencies → 17 Cyrius stdlib modules + 0 external.
- **Binary**: 4.0 MB → 181 KB (96% smaller).

### Breaking

- Language changed from Rust to Cyrius. Consumers must use Cyrius 4.2.0+ to build.
- HTTP server is synchronous (single-threaded). No concurrent request handling.
- MCP tool call forwarding returns error stub — blocked on bote Cyrius port.
- Firewall MCP tools not available — blocked on nein Cyrius port.

## [0.6.0] - 2026-04-03

### Added

- `http-forward` feature gate — external MCP tool forwarding via reqwest is now opt-in.
- `Serialize`/`Deserialize` on 8 previously non-serializable types.
- Input validation: positive CPU/memory on scheduler node registration, cron bounds.
- 13 new tests (305 total), 4 new benchmarks (19 total).

### Fixed

- **Security**: External MCP tool-not-found → 400 (was leaking 404).
- **Security**: Firewall table filter exact match (was substring).
- **Safety**: `setrlimit` return values checked, scheduler `.get()` instead of index, RpcRouter mutex handling.

### Changed

- Binary size: 12 MB → 4.0 MB (−64%) default, 8.2 MB (−32%) with http-forward.
- Dependencies: 354 → 193 (−45%). Dropped anyhow, async-trait.
- `Supervisor::check_health` now generic.

## [0.5.0] - 2026-03-26

### Added

- Full axum HTTP API router with 20+ endpoints.
- Integration test suite (28 tests), benchmark suite (15 benchmarks).

### Fixed

- Restrictive CORS, timestamp correctness, cron time matching, socket permission logging.

## [0.1.0] - 2026-03-25

### Added

- Initial scaffold — all modules extracted from agnosticos monorepo.
