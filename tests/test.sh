#!/bin/sh
# daimon test runner
# Usage: sh tests/test.sh
set -e

echo "=== daimon tests ==="
mkdir -p build
cyrius build tests/daimon.tcyr build/daimon_test
build/daimon_test
TEST_EXIT=$?
rm -f build/daimon_test

echo ""
echo "=== libro MCP tools integration smoke ==="
# The .tcyr unit suite is self-contained (inlines src); the bote/libro
# audit tools only exist in the linked binary, so this exercises the real
# HTTP path: /v1/mcp/tools must advertise the 5 builtins, and dispatching
# them must run bote's handlers over daimon's seeded libro chain.
LIBRO_PORT=18077
LIBRO_OK=1
cyrius build src/main.cyr build/daimon >/dev/null 2>&1 || LIBRO_OK=0
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
libro_check() {
    printf "  %s: " "$2"
    if printf '%s' "$1" | grep -q "$3"; then echo "PASS"; else echo "FAIL"; LIBRO_OK=0; fi
}
libro_check "$LIBRO_MANIFEST" "manifest advertises 5 tools" '"count":5'
libro_check "$LIBRO_MANIFEST" "libro_query listed"          'libro_query'
libro_check "$LIBRO_EXPORT"   "export returns genesis entry" 'mcp.host.init'
libro_check "$LIBRO_VERIFY"   "verify: chain integrity ok"   '"ok":true'
libro_check "$LIBRO_QUERY"    "query filters by severity"    'mcp.host.init'
libro_check "$LIBRO_AUDIT"    "audit records SSRF-rejected registration" 'mcp.register.reject'
if [ $LIBRO_OK -ne 1 ]; then echo "  libro smoke FAILED"; TEST_EXIT=1; fi

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
if [ $TRACE_OK -ne 1 ]; then echo "  trace smoke FAILED"; TEST_EXIT=1; fi

echo ""
echo "=== fuzz harnesses ==="
for f in fuzz/*.fcyr; do
    name=$(basename "$f" .fcyr)
    printf "  %s: " "$name"
    cyrius build "$f" "build/fz_$name" 2>/dev/null && timeout 5 "build/fz_$name" && echo "PASS" || echo "FAIL"
    rm -f "build/fz_$name"
done

exit $TEST_EXIT
