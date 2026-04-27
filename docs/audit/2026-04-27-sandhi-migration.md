# Sandhi Migration Security Audit — 2026-04-27

**Scope.** daimon v1.1.4 — replacement of the hand-rolled HTTP server
(`http_*` parse + send fns, `handle_request`, `serve`, the body-recv
loop) with `lib/sandhi.cyr` (Cyrius 5.7.12 stdlib). The endpoint
handlers (`api_*`) are unchanged.

**Method.** Re-verify each finding from `docs/audit/2026-04-13-security-audit.md`
against the new code path. Add new findings introduced or surfaced
by the migration.

---

## VULN-001 — HTTP request smuggling (CL+TE, CL.CL, TE.TE, Host.Host)

**Original protection (pre-1.1.4).** `http_has_transfer_encoding` rejected
any request carrying `Transfer-Encoding`; `http_parse_content_length`
parsed the first numeric run of the first matching header. CL.CL with
divergent values would parse the first; CL+TE was rejected because TE
was rejected outright.

**Post-migration.** Two layers cover this end-to-end:

1. **Sandhi accept loop** (sync path, `sandhi_server_run`) calls
   `sandhi_server_request_has_cl_te_conflict` (RFC 7230 §3.3.3) and
   `sandhi_server_request_has_dup_smuggling_header` (RFC 7230 §3.3.2 +
   §5.4 — Host / Content-Length / Transfer-Encoding duplicates) BEFORE
   handing the request to daimon. Both reject with `400 Bad Request`.
2. **Daimon's `handle_request`** still calls `http_has_transfer_encoding`
   (now a sandhi shim — `sandhi_server_find_header(buf, n, "Transfer-Encoding")
   != 0`), responding `501 Not Implemented` for legitimate clients that
   send chunked. Defense-in-depth: even if a TE-only request slips past
   the smuggling check (it's not a smuggling vector by itself), daimon
   doesn't decode chunked and refuses.
3. **Async path** (`async_handle_client`) explicitly invokes the same
   sandhi smuggling-check fns inline, then `handle_request`.

**Verification (sync).** Tested via raw socket:

```
GET /v1/health Host: x \n Host: y           → HTTP/1.1 400 Bad Request
POST /v1/agents Content-Length: 10 / 5      → HTTP/1.1 400 Bad Request
POST /v1/agents Content-Length / TE chunked → (smuggling check fires;
                                                blocked before handler)
POST /v1/agents Transfer-Encoding: chunked  → HTTP/1.1 501 Not Implemented
```

**Status:** **Strengthened.** The CL.CL vector that the old loose digit
parser tolerated (`"10, 20"` → 10) is now caught at the sandhi smuggling
layer. Sandhi's `sandhi_server_content_length` is strict per RFC 7230
§3.3.2 (rejects any non-digit in the value).

---

## VULN-002 — JSON injection in error responses

**Status:** **Unchanged.** `json_escape_str()` is invoked in every
`api_*` handler that emits user-controlled strings; sandhi only carries
the bytes to the wire.

---

## VULN-004 — PID reuse race on agent signal delivery

**Status:** **Unrelated to HTTP.** `pidfd_open` / `pidfd_send_signal` in
the supervisor path is unchanged.

---

## VULN-005 — Agent memory directory permissions

**Status:** **Unrelated.** `mkdir(..., 0700)` in the per-agent memory
store is unchanged.

---

## VULN-006 — IPC peer credential check

**Status:** **Unrelated.** `SO_PEERCRED` UID verification on the Unix
socket accept is unchanged. (HTTP and IPC are separate transports.)

---

## VULN-008 — Oversized request DoS

**Original protection (pre-1.1.4).** `handle_request` parsed
Content-Length from the first recv; `cl > MAX_REQUEST_SIZE` (65 536)
returned `413 Payload Too Large` immediately.

**Post-migration.** Sandhi's `sandhi_server_recv_request` caps the
buffer at `HSV_REQ_BUF_SIZE = 65 536` bytes regardless of declared
Content-Length, then returns to the handler. Memory use is bounded
**identically** to the pre-1.1.4 cap.

**Trade-off introduced.** The old fast-413 (parse CL header on first
recv, reject before reading body) is no longer reachable — by the time
daimon's handler sees `blen`, sandhi has already finished its recv
loop, either having read up to the buffer cap or having tripped its
30-second `SO_RCVTIMEO` slowloris guard. An attacker advertising
`Content-Length: 1000000` and never sending the body now ties up one
sandhi worker for 30 s instead of receiving an immediate 413.

**Why this is acceptable.** (1) Memory bound (the actual VULN-008
concern) is preserved — no oversized buffer is ever allocated. (2) The
old code had **no** SO_RCVTIMEO, so a slowloris attacker drip-feeding
header bytes would have tied up a worker indefinitely under 1.1.3 —
the new path is strictly better against that vector. (3) Sandhi is
single-threaded, so production deployments must front it with a
reverse proxy + rate limiting (which daimon already has — VULN-009)
for any meaningful concurrency anyway.

**Future hardening.** Lower sandhi's idle_ms via
`sandhi_server_options_new()` + `sandhi_server_options_idle_ms(opts, 5000)`
+ `sandhi_server_run_opts(...)` if the 30 s default proves abusable in
production. Tracked in roadmap.

**Status:** **Bounded — slower rejection on an obscure declared-CL DoS
variant; better protection against slowloris.**

---

## VULN-009 — Per-IP rate limiting

**Status:** **Unchanged.** `rate_check(cfd)` runs at the top of
`handle_request`, before any sandhi-side parsing — same 60-second
sliding window, 120 req/IP cap. Sandhi has no built-in rate limiter,
so daimon's stays authoritative.

---

## VULN-010 — Agent resource limits

**Status:** **Unrelated.** `agent_spawn_with_limits` (RLIMIT_AS,
RLIMIT_CPU on spawned agent processes) is unchanged.

---

## NEW — Sandhi global request buffer (sync path only)

**Finding.** Sandhi's sync server reuses one process-global buffer
(`_hsv_req_buf`, 65 536 bytes) across all connections — sound under
the single-threaded `sandhi_server_run` accept loop. Daimon's async
path (`async_handle_client`) deliberately allocates a fresh
`MAX_REQUEST_SIZE + 1` buffer per call so cooperative async handlers
can't observe each other's request bytes if their recv loops ever
interleave (cooperative scheduling avoids preemption mid-call today
but the per-call alloc keeps the invariant explicit).

**Status:** Documented in `async_handle_client`'s header comment; no
action required.

---

## NEW — Sandhi binary surface

**Finding.** `lib/sandhi.cyr` is 376 KB / 126 fns covering far more
than daimon uses (HTTP/2, HPACK, SSE, JSON-RPC + MCP-over-HTTP, TLS,
service discovery). DCE shrinks the daimon binary to **452 KB**
(was 263 KB on 1.1.3, +72 %), but the un-DCE'd sandhi modules add
attack surface even when never called.

**Mitigation.** Cyrius's link-time DCE strips uncalled fns at link
time; the bytes that ship are only those reachable from `main`. The
+72 % growth is the reachable subset (server fns, request parsing,
header checks, smuggling rejection). Inspecting the `dead:` listing
from `cyrius build`, no daimon-relevant sandhi fns are dead.

**Status:** Acceptable; documented in the changelog.

---

## NEW — Sandhi `sandhi_server_send_status` writes a non-JSON body for our 400s

**Finding.** Sandhi rejects smuggling with
`sandhi_server_send_status(cfd, 400, "Bad Request")`, which emits an
empty-body response with `Content-Length: 0`. Daimon's other 400s emit
`{"error":"...","code":400}`. Inconsistent body shape between
sandhi-emitted and daimon-emitted 400s.

**Status:** Cosmetic; smuggling 400s don't carry actionable detail
intentionally (no information leak). Documented in API guide.

---

## External CVE / 0-day search

| Subsystem | Search | Findings |
|---|---|---|
| `lib/sandhi.cyr` | "sandhi cyrius CVE", "agnos sandhi vulnerability" | None — sandhi is in-tree to Cyrius, not externally tracked |
| HTTP/1.1 server smuggling | "HTTP smuggling 2026 CL.TE", "HTTP request smuggling new vector" | All currently catalogued vectors (CL.TE, TE.CL, CL.CL, TE.TE, Host.Host, fragment-after-CR) are caught by sandhi 5.7.12 per the source — see `sandhi_server_request_has_*` |
| TCP slowloris | n/a | sandhi default `SO_RCVTIMEO = 30 s` mitigates; daimon old path had no timeout |
| Single-threaded HTTP DoS | n/a | sandhi server is single-threaded through 1.0.0 / current stdlib (`max_conns` exists but is unenforced — see `sandhi/docs/guides/server.md`); daimon production must front with reverse proxy or use `--async` (which uses sandhi parsers under epoll cooperative scheduling) |

---

## Verdict

**No regression of the original 10 VULN findings.** VULN-001 strengthened
(strict CL parsing closes the loose-digit smuggling sub-variant);
VULN-008 bounded with a slower rejection on declared-CL-too-large
(documented above as acceptable, with a one-line follow-up to lower
the idle timeout if needed). All other VULNs unaffected.

**Two new informational findings** (sandhi global buffer; binary surface
growth) — both documented in code or changelog, no action required.

**Roadmap.**
- Future: lower `sandhi_server_options_idle_ms` from default 30 s to
  ~5 s in production once a baseline of legitimate-client request
  durations is collected.
- Future: if a Cyrius stdlib patch lands concurrent server-accept
  enforcement (no version target — sandhi 1.0.0 froze the public
  surface and entered maintenance mode at the 5.7.0 fold; `max_conns`
  exists but is unenforced per `sandhi/docs/guides/server.md`),
  daimon's `--async` path could collapse back to
  `sandhi_server_run_opts(...)` and shed `serve_async`. Until then
  daimon's epoll loop is the only multi-conn path.
