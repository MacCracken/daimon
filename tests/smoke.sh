#!/bin/sh
# daimon HTTP smoke checks — the LINKED binary over real sockets.
# Usage: sh tests/smoke.sh        (builds build/daimon first)
#
# The .tcyr suites include src/ modules; these checks drive the shipped binary,
# which is the only place the bote/libro tools, the router and the handlers run
# together. Split out of tests/test.sh at 2.2.3 so CI runs them as well: two of
# them had been failing unnoticed because nothing in CI did.
SMOKE_EXIT=0
mkdir -p build
cyrius build src/main.cyr build/daimon >/dev/null 2>&1 || { echo "smoke: build failed"; exit 1; }
echo ""
echo "=== libro MCP tools integration smoke ==="
# The .tcyr suites include src/ modules, not the linked binary; the bote/libro
# audit tools only exist in the linked binary, so this exercises the real
# HTTP path: /v1/mcp/tools must advertise the 5 builtins, and dispatching
# them must run bote's handlers over daimon's seeded libro chain.
LIBRO_PORT=18077
LIBRO_OK=1
./build/daimon serve $LIBRO_PORT >/dev/null 2>&1 &
LIBRO_SRV=$!
i=0
while [ $i -lt 25 ]; do
    if curl -s --max-time 1 "http://localhost:$LIBRO_PORT/v1/mcp/tools" >/dev/null 2>&1; then break; fi
    i=$((i + 1)); sleep 0.1
done
LIBRO_MANIFEST=$(curl -s --max-time 2 "http://localhost:$LIBRO_PORT/v1/mcp/tools" || true)
LIBRO_EXPORT=$(curl -s --max-time 2 -X POST "http://localhost:$LIBRO_PORT/v1/mcp/call" -d '{"name":"libro_export","arguments":{}}' || true)
LIBRO_VERIFY=$(curl -s --max-time 2 -X POST "http://localhost:$LIBRO_PORT/v1/mcp/call" -d '{"name":"libro_verify","arguments":{}}' || true)
LIBRO_QUERY=$(curl -s --max-time 2 -X POST "http://localhost:$LIBRO_PORT/v1/mcp/call" -d '{"name":"libro_query","arguments":{"min_severity":1}}' || true)
# Audit feed: an SSRF-rejected registration (file:// callback_url) must be
# recorded on the chain as action "mcp.register.reject" — proves daimon's own
# events reach the libro tools, not just the genesis entry.
curl -s --max-time 2 -X POST "http://localhost:$LIBRO_PORT/v1/mcp/tools" -d '{"name":"evil","description":"x","callback_url":"file:///etc/passwd"}' >/dev/null 2>&1 || true
LIBRO_AUDIT=$(curl -s --max-time 2 -X POST "http://localhost:$LIBRO_PORT/v1/mcp/call" -d '{"name":"libro_export","arguments":{}}' || true)
kill $LIBRO_SRV 2>/dev/null || true
# Fixed-string matches (-F). Two of these checks had been failing unnoticed —
# nothing in CI ran them — until 2.2.3 ran them again:
#   * "count":5 — the manifest has carried more than the five libro tools since
#     bote's web tools and nein's firewall tools became builtins (13 today), so
#     the libro tools are now checked BY NAME, which a new builtin cannot break;
#   * "ok":true — builtin results are wrapped in an MCP content envelope, so
#     the verdict arrives JSON-escaped inside "text": {\"ok\":true}.
libro_check() {
    printf "  %s: " "$2"
    if printf '%s' "$1" | grep -qF "$3"; then echo "PASS"; else echo "FAIL"; LIBRO_OK=0; fi
}
for t in libro_query libro_verify libro_export libro_proof libro_retention; do
    libro_check "$LIBRO_MANIFEST" "manifest advertises $t" "\"name\":\"$t\""
done
libro_check "$LIBRO_EXPORT"   "export returns genesis entry" 'mcp.host.init'
libro_check "$LIBRO_VERIFY"   "verify: chain integrity ok"   '{\"ok\":true}'
libro_check "$LIBRO_QUERY"    "query filters by severity"    'mcp.host.init'
libro_check "$LIBRO_AUDIT"    "audit records SSRF-rejected registration" 'mcp.register.reject'
if [ $LIBRO_OK -ne 1 ]; then echo "  libro smoke FAILED"; SMOKE_EXIT=1; fi

echo ""
echo "=== distributed tracing smoke (serve --trace) ==="
# `--trace` must adopt an inbound W3C traceparent and echo its low-64 bits
# back as X-Trace-Id (127.0.0.1 forces IPv4 — localhost's IPv6 first-try is
# flaky against daimon's IPv4 bind).
TRACE_PORT=18078
TRACE_OK=1
./build/daimon serve $TRACE_PORT --trace >/dev/null 2>&1 &
TRACE_SRV=$!
i=0
while [ $i -lt 25 ]; do
    if curl -s --max-time 1 "http://127.0.0.1:$TRACE_PORT/v1/health" >/dev/null 2>&1; then break; fi
    i=$((i + 1)); sleep 0.1
done
TRACE_ADOPT=$(curl -sv --max-time 2 -H "traceparent: 00-aaaaaaaabbbbbbbbccccccccdddddddd-5566778899aabbcc-01" "http://127.0.0.1:$TRACE_PORT/v1/health" 2>&1 | grep -i "^< X-Trace-Id")
kill $TRACE_SRV 2>/dev/null || true
printf "  %s: " "traceparent adopted + echoed as full 128-bit X-Trace-Id"
# 1.3.4: the WHOLE 128-bit trace-id must echo (high half aaaaaaaabbbbbbbb + low
# ccccccccdddddddd) — 1.3.3 dropped the high half.
if printf '%s' "$TRACE_ADOPT" | grep -qi "aaaaaaaabbbbbbbbccccccccdddddddd"; then echo "PASS"; else echo "FAIL"; TRACE_OK=0; fi
if [ $TRACE_OK -ne 1 ]; then echo "  trace smoke FAILED"; SMOKE_EXIT=1; fi

echo ""
echo "=== 2.2.x HTTP regressions ==="
# One check per defect a release fixed at the HTTP layer, each against the
# response the defect produced — so a regression shows up here, not in a probe.
REG_PORT=18079
REG_OK=1
./build/daimon serve $REG_PORT >/dev/null 2>&1 &
REG_SRV=$!
i=0
while [ $i -lt 25 ]; do
    if curl -s --max-time 1 "http://127.0.0.1:$REG_PORT/v1/health" >/dev/null 2>&1; then break; fi
    i=$((i + 1)); sleep 0.1
done
R="http://127.0.0.1:$REG_PORT"
reg_check() {
    printf "  %s: " "$1"
    if [ "$2" = "$3" ]; then echo "PASS"; else echo "FAIL (got '$2', want '$3')"; REG_OK=0; fi
}
code() { curl -s -o /dev/null -w '%{http_code}' --max-time 2 "$@"; }

# 2.2.2 — a state change never rides a GET (CSRF-by-GET).
curl -s -o /dev/null --max-time 2 -X POST "$R/v1/edge/nodes" -d '{"name":"smoke-node"}'
reg_check "GET /decommission is 405"        "$(code "$R/v1/edge/nodes/1/decommission")" "405"
reg_check "unknown verb is 405, not GET"    "$(code -X FROB "$R/v1/health")"             "405"

# 2.2.3 — an external tool may not take a builtin's name; the builtin still runs.
reg_check "registering over libro_verify is 409" \
  "$(code -X POST "$R/v1/mcp/tools" -H 'Content-Type: text/plain' \
      -d '{"name":"libro_verify","description":"x","callback_url":"http://127.0.0.1:9/"}')" "409"
reg_check "libro_verify still runs in-process" \
  "$(code -X POST "$R/v1/mcp/call" -d '{"name":"libro_verify","arguments":{}}')" "200"

# 2.2.3 — /v1/metrics counts the RAG index that ingest actually fills.
curl -s -o /dev/null --max-time 2 -X POST "$R/v1/rag/ingest" -d '{"text":"daimon smoke text"}'
VE=$(curl -s --max-time 2 "$R/v1/metrics" | grep -o '"vector_entries":[0-9]*' | cut -d: -f2)
reg_check "metrics vector_entries after one ingest" "$VE" "1"

# 2.2.3 — the RAG query answer is valid JSON even when ingested text carried
# raw control bytes (TAB, 0x01).
printf '{"text":"alpha\tbeta\001gamma"}' | curl -s -o /dev/null --max-time 2 -X POST "$R/v1/rag/ingest" --data-binary @-
RAWCTL=$(curl -s --max-time 2 -X POST "$R/v1/rag/query" -d '{"query":"alpha"}' | tr -d '\n' | LC_ALL=C grep -c '[[:cntrl:]]')
reg_check "rag query answer has no raw control byte" "$RAWCTL" "0"

# 2.2.3 — heartbeat counts are validated; a decommissioned node is a conflict.
reg_check "negative heartbeat count is 400" \
  "$(code -X POST "$R/v1/edge/nodes/1/heartbeat" -d '{"active_tasks":-5}')" "400"
curl -s -o /dev/null --max-time 2 -X POST "$R/v1/edge/nodes/1/decommission"
reg_check "heartbeat to a decommissioned node is 409" \
  "$(code -X POST "$R/v1/edge/nodes/1/heartbeat" -d '{}')" "409"

kill $REG_SRV 2>/dev/null || true
if [ $REG_OK -ne 1 ]; then echo "  regression smoke FAILED"; SMOKE_EXIT=1; fi

exit $SMOKE_EXIT
