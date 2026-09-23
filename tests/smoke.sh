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
# 127.0.0.1, not localhost, throughout: curl tries ::1 first and daimon binds
# IPv4 only (the trace section below learned that first).
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
    if curl -s --max-time 1 "http://127.0.0.1:$LIBRO_PORT/v1/mcp/tools" >/dev/null 2>&1; then break; fi
    i=$((i + 1)); sleep 0.1
done
LIBRO_MANIFEST=$(curl -s --max-time 2 "http://127.0.0.1:$LIBRO_PORT/v1/mcp/tools" || true)
LIBRO_EXPORT=$(curl -s --max-time 2 -X POST "http://127.0.0.1:$LIBRO_PORT/v1/mcp/call" -d '{"name":"libro_export","arguments":{}}' || true)
LIBRO_VERIFY=$(curl -s --max-time 2 -X POST "http://127.0.0.1:$LIBRO_PORT/v1/mcp/call" -d '{"name":"libro_verify","arguments":{}}' || true)
LIBRO_QUERY=$(curl -s --max-time 2 -X POST "http://127.0.0.1:$LIBRO_PORT/v1/mcp/call" -d '{"name":"libro_query","arguments":{"min_severity":1}}' || true)
# Audit feed: an SSRF-rejected registration (file:// callback_url) must be
# recorded on the chain as action "mcp.register.reject" — proves daimon's own
# events reach the libro tools, not just the genesis entry.
curl -s --max-time 2 -X POST "http://127.0.0.1:$LIBRO_PORT/v1/mcp/tools" -d '{"name":"evil","description":"x","callback_url":"file:///etc/passwd"}' >/dev/null 2>&1 || true
LIBRO_AUDIT=$(curl -s --max-time 2 -X POST "http://127.0.0.1:$LIBRO_PORT/v1/mcp/call" -d '{"name":"libro_export","arguments":{}}' || true)
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

echo ""
echo "=== 2.3.0 agent lifecycle (serve --agents-dir) ==="
# Fixture agents in a private directory: User traps SIGTERM and exits 0 (and
# records whether it holds any socket), System exits 3 at once, and there is
# deliberately no Service executable and no generic runner.
LC_PORT=18081
LC_OK=1
LC_DIR=$(mktemp -d)
cat > "$LC_DIR/agnos-agent-user-agent" <<'AGENT'
#!/bin/sh
ls -l /proc/$$/fd | grep -c socket > "$0.sockets"
trap 'exit 0' TERM
: > "$0.ready.$$"
while :; do sleep 0.05; done
AGENT
printf '#!/bin/sh\nexit 3\n' > "$LC_DIR/agnos-agent-system-agent"
chmod +x "$LC_DIR"/agnos-agent-*
./build/daimon serve $LC_PORT --agents-dir "$LC_DIR" >/dev/null 2>&1 &
LC_SRV=$!
i=0
while [ $i -lt 25 ]; do
    if curl -s --max-time 1 "http://127.0.0.1:$LC_PORT/v1/health" >/dev/null 2>&1; then break; fi
    i=$((i + 1)); sleep 0.1
done
L="http://127.0.0.1:$LC_PORT"
lc_check() {
    printf "  %s: " "$1"
    if [ "$2" = "$3" ]; then echo "PASS"; else echo "FAIL (got '$2', want '$3')"; LC_OK=0; fi
}
field() { grep -o "\"$1\":[^,}]*" | head -1 | cut -d: -f2 | tr -d '"'; }
lcode() { curl -s -o /dev/null -w '%{http_code}' --max-time 10 "$@"; }

lc_check "register a User agent is 201" "$(lcode -X POST "$L/v1/agents" -d '{"name":"smoke-user","type":"User"}')" "201"
START=$(curl -s --max-time 10 -X POST "$L/v1/agents/1/start")
lc_check "start runs it (status 2)" "$(printf '%s' "$START" | field status)" "2"
APID=$(printf '%s' "$START" | field pid)
i=0; while [ $i -lt 50 ] && [ ! -e "$LC_DIR/agnos-agent-user-agent.ready.$APID" ]; do i=$((i + 1)); sleep 0.1; done
# One socket since 2.3.3: its channel, fd 3. Before, none (and before 2.3.0,
# daimon's listener and the client connection that asked for the start).
lc_check "the agent holds one socket, its channel (listener + client conn not inherited)" \
  "$(cat "$LC_DIR/agnos-agent-user-agent.sockets" 2>/dev/null)" "1"
lc_check "start again is 409"         "$(lcode -X POST "$L/v1/agents/1/start")"   "409"
lc_check "GET .../start is 405"       "$(lcode "$L/v1/agents/1/start")"           "405"
lc_check "pause (status 3)"           "$(curl -s --max-time 10 -X POST "$L/v1/agents/1/pause" | field status)"  "3"
lc_check "resume (status 2)"          "$(curl -s --max-time 10 -X POST "$L/v1/agents/1/resume" | field status)" "2"
lc_check "delete while running is 409" "$(lcode -X DELETE "$L/v1/agents/1")"      "409"
STOP=$(curl -s --max-time 10 -X POST "$L/v1/agents/1/stop")
lc_check "stop ends it (status 5)"    "$(printf '%s' "$STOP" | field status)"     "5"
lc_check "stop was graceful (exit_code 0, not SIGKILL's 137)" "$(printf '%s' "$STOP" | field exit_code)" "0"
lc_check "delete a stopped agent"     "$(lcode -X DELETE "$L/v1/agents/1")"       "200"
lc_check "a deleted agent is 404"     "$(lcode "$L/v1/agents/1")"                 "404"

curl -s -o /dev/null --max-time 10 -X POST "$L/v1/agents" -d '{"name":"smoke-crash","type":"System"}'
curl -s -o /dev/null --max-time 10 -X POST "$L/v1/agents/2/start"
sleep 0.3
CRASH=$(curl -s --max-time 10 "$L/v1/agents/2")
lc_check "an agent that exits on its own is FAILED (status 6)" "$(printf '%s' "$CRASH" | field status)" "6"
lc_check "... with its exit code"     "$(printf '%s' "$CRASH" | field exit_code)" "3"

curl -s -o /dev/null --max-time 10 -X POST "$L/v1/agents" -d '{"name":"smoke-svc","type":"Service"}'
lc_check "no executable for the type is 422" "$(lcode -X POST "$L/v1/agents/3/start")" "422"
lc_check "an unknown type is 400"     "$(lcode -X POST "$L/v1/agents" -d '{"name":"x","type":"root"}')" "400"
# Browser-originated agent control is refused: the text/plain POST a web page
# can send with no CORS preflight (verified to START an agent before 2.3.0).
curl -s -o /dev/null --max-time 10 -X POST "$L/v1/agents" -d '{"name":"smoke-csrf","type":"User"}'
lc_check "a cross-site text/plain start is 403" \
  "$(lcode -X POST -H 'Content-Type: text/plain' -H 'Origin: https://attacker.example' "$L/v1/agents/4/start")" "403"
lc_check "... and nothing was started (status 0)" "$(curl -s --max-time 10 "$L/v1/agents/4" | field status)" "0"
lc_check "a DELETE carrying Origin is 403" \
  "$(lcode -X DELETE -H 'Origin: https://attacker.example' "$L/v1/agents/4")" "403"
lc_check "an unknown agent is 404"    "$(lcode -X POST "$L/v1/agents/999/start")" "404"
AUDIT=$(curl -s --max-time 10 -X POST "$L/v1/mcp/call" -d '{"name":"libro_export","arguments":{}}')
printf "  %s: " "the audit chain records spawn, stop and exit"
if printf '%s' "$AUDIT" | grep -q 'agent.spawn' && printf '%s' "$AUDIT" | grep -q 'agent.stop' \
   && printf '%s' "$AUDIT" | grep -q 'agent.exit'; then echo "PASS"; else echo "FAIL"; LC_OK=0; fi

kill $LC_SRV 2>/dev/null || true
rm -rf "$LC_DIR"
if [ $LC_OK -ne 1 ]; then echo "  lifecycle smoke FAILED"; SMOKE_EXIT=1; fi

echo ""
echo "=== 2.3.0 bind address ==="
# Through 2.2.3 daimon bound 0.0.0.0 although config listen_addr said
# 127.0.0.1. Read the listening socket from /proc/net/tcp (no ss needed):
# local address in hex, 0100007F = 127.0.0.1, state 0A = LISTEN.
BIND_OK=1
bind_check() {
    printf "  %s: " "$1"
    if [ "$2" = "$3" ]; then echo "PASS"; else echo "FAIL (got '$2', want '$3')"; BIND_OK=0; fi
}
listening_on() { awk -v a="$1" '$2 == a && $4 == "0A" { n++ } END { print n + 0 }' /proc/net/tcp; }
BP=18082
BPHEX=$(printf '%04X' $BP)
./build/daimon serve $BP >/dev/null 2>&1 &
BSRV=$!
i=0
while [ $i -lt 25 ]; do
    if curl -s --max-time 1 "http://127.0.0.1:$BP/v1/health" >/dev/null 2>&1; then break; fi
    i=$((i + 1)); sleep 0.1
done
bind_check "default bind is 127.0.0.1" "$(listening_on "0100007F:$BPHEX")" "1"
bind_check "... not 0.0.0.0"           "$(listening_on "00000000:$BPHEX")" "0"
kill $BSRV 2>/dev/null || true
BP2=18083
BP2HEX=$(printf '%04X' $BP2)
./build/daimon serve $BP2 --listen 0.0.0.0 >/dev/null 2>&1 &
BSRV2=$!
i=0
while [ $i -lt 25 ]; do
    if curl -s --max-time 1 "http://127.0.0.1:$BP2/v1/health" >/dev/null 2>&1; then break; fi
    i=$((i + 1)); sleep 0.1
done
bind_check "--listen 0.0.0.0 opens every interface" "$(listening_on "00000000:$BP2HEX")" "1"
kill $BSRV2 2>/dev/null || true
./build/daimon serve 18084 --listen not-an-address >/dev/null 2>&1
bind_check "a malformed --listen exits 1" "$?" "1"
if [ $BIND_OK -ne 1 ]; then echo "  bind smoke FAILED"; SMOKE_EXIT=1; fi

echo ""
echo "=== 2.3.1 task start / complete ==="
# A node with room for exactly one default task (1 cpu, 256 MB) and two tasks:
# the second can only be placed once the first has completed and its
# reservation is back — which is what complete has to do.
TK_PORT=18085
TK_OK=1
./build/daimon serve $TK_PORT >/dev/null 2>&1 &
TK_SRV=$!
i=0
while [ $i -lt 25 ]; do
    if curl -s --max-time 1 "http://127.0.0.1:$TK_PORT/v1/health" >/dev/null 2>&1; then break; fi
    i=$((i + 1)); sleep 0.1
done
K="http://127.0.0.1:$TK_PORT"
tk_check() {
    printf "  %s: " "$1"
    if [ "$2" = "$3" ]; then echo "PASS"; else echo "FAIL (got '$2', want '$3')"; TK_OK=0; fi
}
tfield() { grep -o "\"$1\":[^,}]*" | head -1 | cut -d: -f2 | tr -d '"'; }
tcode() { curl -s -o /dev/null -w '%{http_code}' --max-time 10 "$@"; }
curl -s -o /dev/null --max-time 10 -X POST "$K/v1/scheduler/nodes" -d '{"node_id":"smoke-n1","total_cpu":1,"total_memory_mb":256}'
T1=$(curl -s --max-time 10 -X POST "$K/v1/scheduler/tasks" -d '{"name":"first","agent_id":"agent-7"}' | tfield task_id)
sleep 0.01
T2=$(curl -s --max-time 10 -X POST "$K/v1/scheduler/tasks" -d '{"name":"second","agent_id":"agent-8"}' | tfield task_id)
curl -s -o /dev/null --max-time 10 -X POST "$K/v1/scheduler/schedule"
tk_check "the node lists its placed task"  "$(curl -s --max-time 10 "$K/v1/scheduler/nodes/smoke-n1/tasks" | tfield task_id)" "$T1"
tk_check "an unknown node is 404"          "$(tcode "$K/v1/scheduler/nodes/nope/tasks")"          "404"
tk_check "a QUEUED task cannot start (409)" "$(tcode -X POST "$K/v1/scheduler/tasks/$T2/start")"  "409"
tk_check "complete before start is 409"    "$(tcode -X POST "$K/v1/scheduler/tasks/$T1/complete")" "409"
tk_check "start: Running"                  "$(curl -s --max-time 10 -X POST "$K/v1/scheduler/tasks/$T1/start" | tfield status)" "Running"
tk_check "start again is 409"              "$(tcode -X POST "$K/v1/scheduler/tasks/$T1/start")"   "409"
tk_check "GET .../start is 405"            "$(tcode "$K/v1/scheduler/tasks/$T1/start")"           "405"
tk_check "a browser-originated complete is 403" \
  "$(tcode -X POST -H 'Origin: https://attacker.example' "$K/v1/scheduler/tasks/$T1/complete")" "403"
tk_check "a non-terminal status is 400" \
  "$(tcode -X POST "$K/v1/scheduler/tasks/$T1/complete" -d '{"status":"running"}')" "400"
tk_check "stats count it running"          "$(curl -s --max-time 10 "$K/v1/scheduler/stats" | tfield running)" "1"
tk_check "complete (no body): Completed" \
  "$(curl -s --max-time 10 -X POST "$K/v1/scheduler/tasks/$T1/complete" | tfield status)" "Completed"
tk_check "the freed capacity places the waiting task" \
  "$(curl -s --max-time 10 -X POST "$K/v1/scheduler/schedule" | tfield task_id)" "$T2"
curl -s -o /dev/null --max-time 10 -X POST "$K/v1/scheduler/tasks/$T2/start"
FAILED=$(curl -s --max-time 10 -X POST "$K/v1/scheduler/tasks/$T2/complete" -d '{"status":"failed","reason":"disk full"}')
tk_check "complete {status:failed}: Failed" "$(printf '%s' "$FAILED" | tfield status)"      "Failed"
tk_check "... with its reason"             "$(printf '%s' "$FAILED" | tfield fail_reason)" "disk full"
STATS=$(curl -s --max-time 10 "$K/v1/scheduler/stats")
tk_check "stats: one completed"            "$(printf '%s' "$STATS" | tfield completed)"    "1"
tk_check "stats: one failed"               "$(printf '%s' "$STATS" | tfield failed)"       "1"
tk_check "an unknown task is 404"          "$(tcode -X POST "$K/v1/scheduler/tasks/no-such-task/start")" "404"
kill $TK_SRV 2>/dev/null || true
if [ $TK_OK -ne 1 ]; then echo "  task smoke FAILED"; SMOKE_EXIT=1; fi

echo ""
echo "=== 2.3.2 request strings ==="
# Every body used to be read with bayan's flat parser: escapes kept raw, a
# nested object's keys read as top-level fields. Each check sends what the flat
# reader got wrong and matches the RAW response bytes (grep -F, byte locale),
# so nothing between here and daimon re-escapes anything.
JS_PORT=18086
JS_OK=1
./build/daimon serve $JS_PORT >/dev/null 2>&1 &
JS_SRV=$!
i=0
while [ $i -lt 25 ]; do
    if curl -s --max-time 1 "http://127.0.0.1:$JS_PORT/v1/health" >/dev/null 2>&1; then break; fi
    i=$((i + 1)); sleep 0.1
done
J="http://127.0.0.1:$JS_PORT"
js_has() {
    printf "  %s: " "$1"
    if printf '%s' "$2" | LC_ALL=C grep -qF -- "$3"; then echo "PASS"; else echo "FAIL (got '$2')"; JS_OK=0; fi
}
js_code() {
    printf "  %s: " "$1"
    if [ "$2" = "$3" ]; then echo "PASS"; else echo "FAIL (got '$2', want '$3')"; JS_OK=0; fi
}
jcode() { curl -s -o /dev/null -w '%{http_code}' --max-time 10 "$@"; }
# say "hi" \o/ é — sent escaped, stored decoded, echoed escaped exactly once.
js_has "escapes are decoded (quote, backslash, \\u00e9)" \
  "$(curl -s --max-time 10 -X POST "$J/v1/agents" --data-binary '{"name":"say \"hi\" \\o/ \u00e9"}')" \
  '"name":"say \"hi\" \\o/ é"'
js_has "a nested key does not shadow the top-level one" \
  "$(curl -s --max-time 10 -X POST "$J/v1/agents" -d '{"meta":{"x":1,"name":"nested"},"name":"top"}')" '"name":"top"'
js_has "a field after a nested object is read" \
  "$(curl -s --max-time 10 -X POST "$J/v1/agents" -d '{"meta":{"x":1},"name":"after"}')" '"name":"after"'
js_code "a body that is not JSON is 400"   "$(jcode -X POST "$J/v1/agents" -d 'name=formish')" "400"
js_code "a duplicate key is 400"           "$(jcode -X POST "$J/v1/agents" -d '{"name":"a","name":"b"}')" "400"
js_code "U+0000 in a field is 400"         "$(jcode -X POST "$J/v1/agents" --data-binary '{"name":"a\u0000b"}')" "400"
js_code "an escaped-slash callback_url registers (was refused)" \
  "$(jcode -X POST "$J/v1/mcp/tools" --data-binary '{"name":"slashy","description":"d","callback_url":"http:\/\/127.0.0.1:9\/"}')" "201"
js_has "mcp call follows the TOP-LEVEL name, not a nested one" \
  "$(curl -s --max-time 10 -X POST "$J/v1/mcp/call" -d '{"arguments":{"x":1,"name":"libro_export"},"name":"libro_verify"}')" \
  '{\"ok\":true}'
printf '{"text":"alpha\tbeta\001gamma"}' | curl -s -o /dev/null --max-time 10 -X POST "$J/v1/rag/ingest" --data-binary @-
js_has "a control byte comes back escaped (was dropped)" \
  "$(curl -s --max-time 10 -X POST "$J/v1/rag/query" -d '{"query":"alpha"}')" 'beta\u0001gamma'
curl -s -o /dev/null --max-time 10 -X POST "$J/v1/scheduler/nodes" -d '{"node_id":"js-n1","total_cpu":1,"total_memory_mb":256}'
JT=$(curl -s --max-time 10 -X POST "$J/v1/scheduler/tasks" -d '{"name":"j"}' | grep -o '"task_id":"[^"]*"' | cut -d'"' -f4)
curl -s -o /dev/null --max-time 10 -X POST "$J/v1/scheduler/schedule"
curl -s -o /dev/null --max-time 10 -X POST "$J/v1/scheduler/tasks/$JT/start"
js_has "a failure reason round-trips, escaped once" \
  "$(curl -s --max-time 10 -X POST "$J/v1/scheduler/tasks/$JT/complete" --data-binary '{"status":"failed","reason":"disk \"full\" at C:\\temp"}')" \
  '"fail_reason":"disk \"full\" at C:\\temp"'
kill $JS_SRV 2>/dev/null || true
if [ $JS_OK -ne 1 ]; then echo "  request-string smoke FAILED"; SMOKE_EXIT=1; fi

echo ""
echo "=== 2.3.3 agent channels ==="
# A started agent writes frames on fd 3 and reads a reply byte per frame; daimon
# puts what it accepts on the bus at its next request. The fixture sends 101
# broadcasts (one more than a subscriber queue holds), one malformed frame, and
# later one more broadcast and one oversize length, each step gated on a file
# the smoke creates.
IC_PORT=18087
IC_OK=1
IC_DIR=$(mktemp -d)
cat > "$IC_DIR/agnos-agent-user-agent" <<'AGENT'
#!/bin/sh
trap 'exit 0' TERM
echo "$AGNOS_IPC_FD" > "$0.env"
i=0
while [ $i -lt 101 ]; do printf '\000\000\000\016{"target":"*"}' >&3; i=$((i + 1)); done
head -c 101 <&3 | od -v -An -tu1 | tr -s ' ' '\n' | grep -c '^1$' > "$0.acks"
printf '\000\000\000\007{"x":1}' >&3
head -c 1 <&3 | od -v -An -tu1 | tr -d ' ' > "$0.nack"
: > "$0.done1"
while [ ! -e "$0.go" ]; do sleep 0.05; done
printf '\000\000\000\016{"target":"*"}' >&3
head -c 1 <&3 > /dev/null
: > "$0.done2"
while [ ! -e "$0.go2" ]; do sleep 0.05; done
printf '\000\001\000\001' >&3
head -c 1 <&3 | od -v -An -tu1 | tr -d ' ' > "$0.over"
head -c 1 <&3 | wc -c | tr -d ' ' > "$0.eof"
: > "$0.done3"
while :; do sleep 0.05; done
AGENT
chmod +x "$IC_DIR/agnos-agent-user-agent"
IX="$IC_DIR/agnos-agent-user-agent"
./build/daimon serve $IC_PORT --agents-dir "$IC_DIR" >/dev/null 2>&1 &
IC_SRV=$!
i=0
while [ $i -lt 25 ]; do
    if curl -s --max-time 1 "http://127.0.0.1:$IC_PORT/v1/health" >/dev/null 2>&1; then break; fi
    i=$((i + 1)); sleep 0.1
done
C="http://127.0.0.1:$IC_PORT"
ic_check() {
    printf "  %s: " "$1"
    if [ "$2" = "$3" ]; then echo "PASS"; else echo "FAIL (got '$2', want '$3')"; IC_OK=0; fi
}
ic_wait() { i=0; while [ $i -lt 50 ] && [ ! -e "$1" ]; do i=$((i + 1)); sleep 0.1; done; }
metric() { curl -s --max-time 10 "$C/v1/metrics" | grep -o "\"$1\":[0-9]*" | cut -d: -f2; }

curl -s -o /dev/null --max-time 10 -X POST "$C/v1/agents" -d '{"name":"talker","type":"User"}'
curl -s -o /dev/null --max-time 10 -X POST "$C/v1/agents" -d '{"name":"listener","type":"System"}'
curl -s -o /dev/null --max-time 10 -X POST "$C/v1/agents/1/start"
ic_wait "$IX.done1"
ic_check "the agent is told its channel: AGNOS_IPC_FD=3" "$(cat "$IX.env" 2>/dev/null)" "3"
ic_check "101 frames, 101 ACKs"                "$(cat "$IX.acks" 2>/dev/null)" "101"
ic_check "a malformed frame is NACK_INVALID (3)" "$(cat "$IX.nack" 2>/dev/null)" "3"
ic_check "the next request puts all 101 on the bus" "$(metric ipc_messages)" "101"
ic_check "the malformed one is counted as refused" "$(metric ipc_refused)" "1"
# Both registered agents are subscribed, so each queue took 100 and dropped 1.
ic_check "every registered agent's queue got them (1 dropped each)" "$(metric bus_dropped)" "2"
ic_check "delete the agent that never ran" "$(lcode -X DELETE "$C/v1/agents/2")" "200"
: > "$IX.go"
ic_wait "$IX.done2"
ic_check "one more broadcast is published" "$(metric ipc_messages)" "102"
ic_check "a deleted agent's queue is gone (only the talker's drops)" "$(metric bus_dropped)" "3"
: > "$IX.go2"
ic_wait "$IX.done3"
ic_check "an oversize length is NACK_INVALID (3)" "$(cat "$IX.over" 2>/dev/null)" "3"
ic_check "... and the channel is closed (EOF)"   "$(cat "$IX.eof" 2>/dev/null)" "0"
i=0; IC_AUDIT=""
while [ $i -lt 20 ]; do
    IC_AUDIT=$(curl -s --max-time 10 -X POST "$C/v1/mcp/call" -d '{"name":"libro_export","arguments":{}}')
    if printf '%s' "$IC_AUDIT" | grep -q 'ipc.frame.oversize'; then break; fi
    i=$((i + 1)); sleep 0.1
done
printf "  %s: " "the closed channel is on the audit chain (ipc.frame.oversize)"
if printf '%s' "$IC_AUDIT" | grep -q 'ipc.frame.oversize'; then echo "PASS"; else echo "FAIL"; IC_OK=0; fi
ic_check "... and counted as refused" "$(metric ipc_refused)" "2"
ic_check "the agent still stops cleanly" \
  "$(curl -s --max-time 10 -X POST "$C/v1/agents/1/stop" | field exit_code)" "0"
kill $IC_SRV 2>/dev/null || true
rm -rf "$IC_DIR"
if [ $IC_OK -ne 1 ]; then echo "  channel smoke FAILED"; SMOKE_EXIT=1; fi

exit $SMOKE_EXIT
