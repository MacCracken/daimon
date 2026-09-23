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
lc_check "... under its limits (limits_enforced, 2.4.0)" "$(printf '%s' "$START" | field limits_enforced)" "true"
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
echo "=== 2.3.3 / 2.3.4 agent channels and messages ==="
# A started agent writes frames on fd 3 and reads a reply byte per frame; the
# message goes onto daimon's bus at once (2.3.4: daimon's own event loop), and
# the reply says what became of it. The fixture:
#   * sends a frame to the NAME "listener" (another registered agent);
#   * sends 101 broadcasts: every queue holds 100, so the last one has nowhere
#     to go;
#   * sends a malformed frame and one to a target nobody has;
# then, each gated on a file the smoke creates, one more broadcast and an
# oversize length.
IC_PORT=18087
IC_OK=1
IC_DIR=$(mktemp -d)
cat > "$IC_DIR/agnos-agent-user-agent" <<'AGENT'
#!/bin/sh
trap 'exit 0' TERM
echo "$AGNOS_IPC_FD" > "$0.env"
printf '\000\000\000\025{"target":"listener"}' >&3
head -c 1 <&3 | od -v -An -tu1 | tr -d ' ' > "$0.named"
i=0
while [ $i -lt 101 ]; do printf '\000\000\000\016{"target":"*"}' >&3; i=$((i + 1)); done
head -c 101 <&3 | od -v -An -tu1 | tr -s ' ' '\n' | grep -v '^$' > "$0.replies"
printf '\000\000\000\007{"x":1}' >&3
head -c 1 <&3 | od -v -An -tu1 | tr -d ' ' > "$0.nack"
printf '\000\000\000\023{"target":"nobody"}' >&3
head -c 1 <&3 | od -v -An -tu1 | tr -d ' ' > "$0.notarget"
: > "$0.done1"
while [ ! -e "$0.go" ]; do sleep 0.05; done
printf '\000\000\000\016{"target":"*"}' >&3
head -c 1 <&3 | od -v -An -tu1 | tr -d ' ' > "$0.after"
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
jcount() { grep -o "\"count\":[0-9]*" | cut -d: -f2; }

curl -s -o /dev/null --max-time 10 -X POST "$C/v1/agents" -d '{"name":"talker","type":"User"}'
curl -s -o /dev/null --max-time 10 -X POST "$C/v1/agents" -d '{"name":"listener","type":"System"}'
curl -s -o /dev/null --max-time 10 -X POST "$C/v1/agents" -d '{"name":"listener","type":"System"}'
curl -s -o /dev/null --max-time 10 -X POST "$C/v1/agents/1/start"
ic_wait "$IX.done1"
ic_check "the agent is told its channel: AGNOS_IPC_FD=3" "$(cat "$IX.env" 2>/dev/null)" "3"
ic_check "a frame to a registered NAME is ACKed (1)" "$(cat "$IX.named" 2>/dev/null)" "1"
ic_check "100 broadcasts fit every queue: 100 ACKs" "$(grep -c '^1$' "$IX.replies" 2>/dev/null)" "100"
ic_check "the 101st has nowhere to go: NACK_QUEUE_FULL (2)" "$(tail -1 "$IX.replies" 2>/dev/null)" "2"
ic_check "a malformed frame is NACK_INVALID (3)" "$(cat "$IX.nack" 2>/dev/null)" "3"
ic_check "a target nobody has is NACK_NO_TARGET (4)" "$(cat "$IX.notarget" 2>/dev/null)" "4"
ic_check "published at once, no request needed: 101 on the bus" "$(metric ipc_messages)" "101"
ic_check "refused: the full one, the malformed one, the unknown target" "$(metric ipc_refused)" "3"
# Three queues (the talker, the listener, and the second "listener" — an agent
# like any other, just without the name): the 100th broadcast finds the
# listener's full (it also holds the named frame), the 101st finds all three full.
ic_check "dropped at full queues: 1 + 3" "$(metric bus_dropped)" "4"
ic_check "one open channel" "$(metric ipc_channels)" "1"
TAKE2=$(curl -s --max-time 10 -X POST "$C/v1/agents/2/messages/take" -d '{"max":1}')
printf "  %s: " "the name routes to the FIRST agent that registered it (2, not 3)"
if printf '%s' "$TAKE2" | grep -q '"target":"listener"' && printf '%s' "$TAKE2" | grep -q '"source":"1"'; then echo "PASS"; else echo "FAIL ($TAKE2)"; IC_OK=0; fi
ic_check "take reports what is left (99 broadcasts)" "$(printf '%s' "$TAKE2" | grep -o '"remaining":[0-9]*' | cut -d: -f2)" "99"
TAKE3=$(curl -s --max-time 10 -X POST "$C/v1/agents/3/messages/take")
ic_check "the later registrant of the name got the broadcasts (100)" "$(printf '%s' "$TAKE3" | jcount)" "100"
ic_check "... and not the frame sent to the name" "$(printf '%s' "$TAKE3" | grep -c '"target":"listener"')" "0"
ic_check "take is POST only (GET is 405)" "$(lcode "$C/v1/agents/1/messages/take")" "405"
ic_check "a browser may not take messages (403)" \
  "$(lcode -X POST -H 'Origin: https://attacker.example' "$C/v1/agents/1/messages/take")" "403"
ic_check "delete the listener" "$(lcode -X DELETE "$C/v1/agents/2")" "200"
ic_check "the talker's own queue has its 100 broadcasts" \
  "$(curl -s --max-time 10 -X POST "$C/v1/agents/1/messages/take" | jcount)" "100"
ic_check "with every queue emptied, the bus holds 0 bytes (freed, VULN-018)" "$(metric bus_bytes)" "0"
: > "$IX.go"
ic_wait "$IX.done2"
ic_check "room again: the next broadcast is ACKed" "$(cat "$IX.after" 2>/dev/null)" "1"
ic_check "an HTTP client can send to an agent (201)" \
  "$(lcode -X POST "$C/v1/agents/1/messages" -d '{"type":"command","payload":{"k":"v"}}')" "201"
TAKE1=$(curl -s --max-time 10 -X POST "$C/v1/agents/1/messages/take")
ic_check "both are on its queue" "$(printf '%s' "$TAKE1" | jcount)" "2"
printf "  %s: " "an HTTP message's source is \"api\" and its payload is intact"
if printf '%s' "$TAKE1" | grep -q '"source":"api","target":"1","type":"command","payload":{"k":"v"}'; then echo "PASS"; else echo "FAIL ($TAKE1)"; IC_OK=0; fi
ic_check "sending to an agent that does not exist is 404" "$(lcode -X POST "$C/v1/agents/99/messages" -d '{}')" "404"
: > "$IX.go2"
ic_wait "$IX.done3"
ic_check "an oversize length is NACK_INVALID (3)" "$(cat "$IX.over" 2>/dev/null)" "3"
ic_check "... and the channel is closed (EOF)"   "$(cat "$IX.eof" 2>/dev/null)" "0"
IC_AUDIT=$(curl -s --max-time 10 -X POST "$C/v1/mcp/call" -d '{"name":"libro_export","arguments":{}}')
printf "  %s: " "the closed channel is on the audit chain (ipc.frame.oversize)"
if printf '%s' "$IC_AUDIT" | grep -q 'ipc.frame.oversize'; then echo "PASS"; else echo "FAIL"; IC_OK=0; fi
ic_check "... and counted as refused" "$(metric ipc_refused)" "4"
ic_check "no open channel is left" "$(metric ipc_channels)" "0"
ic_check "the agent still stops cleanly" \
  "$(curl -s --max-time 10 -X POST "$C/v1/agents/1/stop" | field exit_code)" "0"
kill $IC_SRV 2>/dev/null || true
rm -rf "$IC_DIR"
if [ $IC_OK -ne 1 ]; then echo "  channel smoke FAILED"; SMOKE_EXIT=1; fi

echo ""
echo "=== 2.3.4 the event loop ==="
# daimon runs its own loop since 2.3.4: a slow client holds only its own
# connection, and a stop waits for its agent, not for the server (VULN-014).
EL_PORT=18088
EL_OK=1
EL_DIR=$(mktemp -d)
cat > "$EL_DIR/agnos-agent-service-agent" <<'AGENT'
#!/bin/sh
trap '' TERM
: > "$0.ready"
while :; do sleep 0.05; done
AGENT
chmod +x "$EL_DIR/agnos-agent-service-agent"
./build/daimon serve $EL_PORT --agents-dir "$EL_DIR" >/dev/null 2>&1 &
EL_SRV=$!
i=0
while [ $i -lt 25 ]; do
    if curl -s --max-time 1 "http://127.0.0.1:$EL_PORT/v1/health" >/dev/null 2>&1; then break; fi
    i=$((i + 1)); sleep 0.1
done
E="http://127.0.0.1:$EL_PORT"
el_check() {
    printf "  %s: " "$1"
    if [ "$2" = "$3" ]; then echo "PASS"; else echo "FAIL (got '$2', want '$3')"; EL_OK=0; fi
}
# A request answered within a second reads 1.
fast() { curl -s -o /dev/null -w '%{time_total}' --max-time 5 "$E/v1/health" | awk '{ print ($1 < 1.0) ? 1 : 0 }'; }
# A client uploading one byte a second.
head -c 40 /dev/zero | tr '\0' 'a' > "$EL_DIR/body"
curl -s -o /dev/null --max-time 10 --limit-rate 1 -X POST "$E/v1/agents" --data-binary @"$EL_DIR/body" &
SLOW=$!
sleep 1
el_check "a request is answered at once while another client trickles its body" "$(fast)" "1"
kill $SLOW 2>/dev/null || true
curl -s -o /dev/null --max-time 10 -X POST "$E/v1/agents" -d '{"name":"stubborn","type":"Service"}'
curl -s -o /dev/null --max-time 10 -X POST "$E/v1/agents/1/start"
i=0; while [ $i -lt 50 ] && [ ! -e "$EL_DIR/agnos-agent-service-agent.ready" ]; do i=$((i + 1)); sleep 0.1; done
# The agent ignores SIGTERM, so its stop takes the whole 5 s grace.
curl -s --max-time 20 -X POST "$E/v1/agents/1/stop" > "$EL_DIR/stop.out" &
STOPPER=$!
sleep 1
el_check "during a 5 s stop, another request is answered at once (VULN-014)" "$(fast)" "1"
el_check "... and sees the agent STOPPING (4)" "$(curl -s --max-time 5 "$E/v1/agents/1" | field status)" "4"
wait $STOPPER
el_check "the stop is answered once the agent is gone: SIGKILL (137)" "$(field exit_code < "$EL_DIR/stop.out")" "137"
el_check "... STOPPED (5)" "$(field status < "$EL_DIR/stop.out")" "5"
kill $EL_SRV 2>/dev/null || true
rm -rf "$EL_DIR"
if [ $EL_OK -ne 1 ]; then echo "  event loop smoke FAILED"; SMOKE_EXIT=1; fi

echo ""
echo "=== 2.3.4 hosts, origins, edge capabilities ==="
HO_PORT=18089
HO_OK=1
./build/daimon serve $HO_PORT >/dev/null 2>&1 &
HO_SRV=$!
i=0
while [ $i -lt 25 ]; do
    if curl -s --max-time 1 "http://127.0.0.1:$HO_PORT/v1/health" >/dev/null 2>&1; then break; fi
    i=$((i + 1)); sleep 0.1
done
H="http://127.0.0.1:$HO_PORT"
ho_check() {
    printf "  %s: " "$1"
    if [ "$2" = "$3" ]; then echo "PASS"; else echo "FAIL (got '$2', want '$3')"; HO_OK=0; fi
}
hcode() { curl -s -o /dev/null -w '%{http_code}' --max-time 10 "$@"; }
# DNS rebinding (VULN-012): the page's requests carry the attacker's name.
ho_check "a request for another host name is refused (DNS rebinding)" \
  "$(hcode -H 'Host: attacker.example:'$HO_PORT "$H/v1/health")" "403"
ho_check "localhost is daimon's own name" "$(hcode -H 'Host: localhost:'$HO_PORT "$H/v1/health")" "200"
# Cross-site writes on every route, not only agent control.
ho_check "a cross-site text/plain POST to /v1/mcp/tools is refused" \
  "$(hcode -X POST -H 'Content-Type: text/plain' -H 'Origin: https://attacker.example' "$H/v1/mcp/tools" \
      -d '{"name":"x","description":"d","callback_url":"http://127.0.0.1:9/"}')" "403"
ho_check "a page on daimon's own loopback host may write" \
  "$(hcode -X POST -H 'Origin: http://127.0.0.1:'$HO_PORT "$H/v1/mcp/tools" \
      -d '{"name":"x","description":"d","callback_url":"http://127.0.0.1:9/"}')" "201"
ho_check "a cross-site GET is still answered (it changes nothing; CORS keeps its answer from the page)" \
  "$(hcode -H 'Origin: https://attacker.example' "$H/v1/health")" "200"
# Edge nodes register their capabilities, numbered on their own.
curl -s -o /dev/null --max-time 10 -X POST "$H/v1/agents" -d '{"name":"a1"}'
curl -s -o /dev/null --max-time 10 -X POST "$H/v1/agents" -d '{"name":"a2"}'
EID=$(curl -s --max-time 10 -X POST "$H/v1/edge/nodes" \
  -d '{"name":"pi-1","arch":"aarch64","cpu_cores":4,"memory_mb":8192,"has_gpu":true}' | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
ho_check "edge ids have their own counter (the first node is 1, after two agents)" "$EID" "1"
EN=$(curl -s --max-time 10 "$H/v1/edge/nodes/$EID")
printf "  %s: " "a node keeps the capabilities it registered"
if printf '%s' "$EN" | grep -q '"arch":"aarch64","cpu_cores":4,"memory_mb":8192,"disk_mb":32768,"has_gpu":true'; then echo "PASS"; else echo "FAIL ($EN)"; HO_OK=0; fi
ho_check "the fleet's stats count them" "$(curl -s --max-time 10 "$H/v1/edge/stats" | grep -o '"cpu_cores":[0-9]*' | cut -d: -f2)" "4"
ho_check "an invalid arch is 422" "$(hcode -X POST "$H/v1/edge/nodes" -d '{"name":"bad","arch":"X86!"}')" "422"
ho_check "cpu_cores must be a positive integer (422)" "$(hcode -X POST "$H/v1/edge/nodes" -d '{"name":"bad2","cpu_cores":0}')" "422"
ho_check "a duplicate edge name is still refused" "$(hcode -X POST "$H/v1/edge/nodes" -d '{"name":"pi-1"}')" "400"
ho_check "without --agent-output capture, the output route says so (409)" "$(hcode "$H/v1/agents/1/output")" "409"
kill $HO_SRV 2>/dev/null || true
./build/daimon serve 18090 --listen 0.0.0.0 >/dev/null 2>&1 &
HO2=$!
i=0; while [ $i -lt 25 ]; do curl -s --max-time 1 "http://127.0.0.1:18090/v1/health" >/dev/null 2>&1 && break; i=$((i + 1)); sleep 0.1; done
ho_check "with --listen 0.0.0.0 the operator chose its names: any Host is served" \
  "$(hcode -H 'Host: daimon.lan:18090' "http://127.0.0.1:18090/v1/health")" "200"
kill $HO2 2>/dev/null || true
./build/daimon serve 18091 --agent-env bogus >/dev/null 2>&1
ho_check "an unknown --agent-env exits 1" "$?" "1"
# Captured output (2.3.4, serve --agent-output capture).
OC_DIR=$(mktemp -d)
printf '#!/bin/sh\ntrap "exit 0" TERM\necho "out line"\necho "err line" >&2\n: > "$0.ready"\nwhile :; do sleep 0.05; done\n' > "$OC_DIR/agnos-agent-user-agent"
chmod +x "$OC_DIR/agnos-agent-user-agent"
./build/daimon serve 18092 --agents-dir "$OC_DIR" --agent-output capture >/dev/null 2>&1 &
OC_SRV=$!
i=0; while [ $i -lt 25 ]; do curl -s --max-time 1 "http://127.0.0.1:18092/v1/health" >/dev/null 2>&1 && break; i=$((i + 1)); sleep 0.1; done
curl -s -o /dev/null --max-time 10 -X POST "http://127.0.0.1:18092/v1/agents" -d '{"name":"talky"}'
curl -s -o /dev/null --max-time 10 -X POST "http://127.0.0.1:18092/v1/agents/1/start"
i=0; while [ $i -lt 50 ] && [ ! -e "$OC_DIR/agnos-agent-user-agent.ready" ]; do i=$((i + 1)); sleep 0.1; done
sleep 0.3
OUTJ=$(curl -s --max-time 10 "http://127.0.0.1:18092/v1/agents/1/output")
printf "  %s: " "with --agent-output capture, an agent's stdout and stderr are served"
if printf '%s' "$OUTJ" | grep -q 'out line' && printf '%s' "$OUTJ" | grep -q 'err line'; then echo "PASS"; else echo "FAIL ($OUTJ)"; HO_OK=0; fi
curl -s -o /dev/null --max-time 10 -X POST "http://127.0.0.1:18092/v1/agents/1/stop"
kill $OC_SRV 2>/dev/null || true
rm -rf "$OC_DIR"
if [ $HO_OK -ne 1 ]; then echo "  host/origin/edge smoke FAILED"; SMOKE_EXIT=1; fi

echo ""
echo "=== detached calls smoke (2.3.4) ==="
# A call that waits on another server (a forwarded MCP call, web_fetch) runs in
# a child process; the loop relays its answer. Before, it held the server: a
# health check sent during a 2 s call waited 1.7 s.
DT_OK=1
dt_check() {
    printf "  %s: " "$1"
    if [ "$2" = "$3" ]; then echo "PASS"; else echo "FAIL (got '$2', expected '$3')"; DT_OK=0; fi
}
if command -v python3 >/dev/null 2>&1; then
    DT_DIR=$(mktemp -d)
    # An MCP endpoint that answers after 2 s: POST a JSON-RPC result naming the
    # method it was asked, GET a page.
    cat > "$DT_DIR/slow_mcp.py" <<'PY'
import json, sys, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_POST(self):
        req = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))) or b"{}")
        time.sleep(2)
        body = json.dumps({"jsonrpc": "2.0", "id": req.get("id"), "result": {"content": [
            {"type": "text", "text": "slow ok: " + str(req.get("method"))}], "isError": False}}).encode()
        self.send_response(200); self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def do_GET(self):
        time.sleep(2)
        body = b"<html><body><p>slow page</p></body></html>"
        self.send_response(200); self.send_header("Content-Type", "text/html")
        self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
PY
    python3 "$DT_DIR/slow_mcp.py" 18094 >/dev/null 2>&1 &
    DT_UP=$!
    ./build/daimon serve 18093 >/dev/null 2>&1 &
    DT_SRV=$!
    DT=http://127.0.0.1:18093
    i=0; while [ $i -lt 25 ]; do curl -s --max-time 1 "$DT/v1/health" >/dev/null 2>&1 && break; i=$((i + 1)); sleep 0.1; done
    curl -s -o /dev/null --max-time 10 -X POST "$DT/v1/mcp/tools" \
        -d '{"name":"slow_tool","description":"d","callback_url":"http://127.0.0.1:18094/"}'
    curl -s -o /dev/null --max-time 10 -X POST "$DT/v1/mcp/resources" \
        -d '{"uri":"test://slow","name":"slow","callback_url":"http://127.0.0.1:18094/"}'
    for what in tool resource web; do
        case $what in
            tool) path=/v1/mcp/call; body='{"name":"slow_tool","arguments":{}}'; want='slow ok: tools/call' ;;
            resource) path=/v1/mcp/resources/read; body='{"uri":"test://slow"}'; want='slow ok: resources/read' ;;
            web) path=/v1/mcp/call; body='{"name":"web_fetch","arguments":{"url":"http://127.0.0.1:18094/page"}}'; want='slow page' ;;
        esac
        curl -s --max-time 20 -o "$DT_DIR/$what.out" -w '%{http_code}' -X POST "$DT$path" -d "$body" > "$DT_DIR/$what.code" &
        DT_CALL=$!
        sleep 0.3
        HT=$(curl -s -o /dev/null --max-time 10 -w '%{time_total}' "$DT/v1/health")
        dt_check "$what: health during a 2 s call answers within 0.5 s" \
            "$(awk -v t="$HT" 'BEGIN { if (t < 0.5) print "yes"; else print "no, " t " s" }')" "yes"
        wait $DT_CALL
        dt_check "$what: the call is answered (200)" "$(cat "$DT_DIR/$what.code")" "200"
        dt_check "$what: ... with the endpoint's answer" "$(grep -c "$want" "$DT_DIR/$what.out")" "1"
    done
    sleep 0.3
    # The children are reaped: none of daimon's is left, zombie or not. (The
    # parent pid is field 2 after the ")" that ends the command name.)
    DT_KIDS=0
    for d in /proc/[0-9]*; do
        pp=$(sed 's/.*) //' "$d/stat" 2>/dev/null | awk '{print $2}')
        if [ "$pp" = "$DT_SRV" ]; then DT_KIDS=$((DT_KIDS + 1)); fi
    done
    dt_check "every call's child is reaped" "$DT_KIDS" "0"
    kill $DT_SRV $DT_UP 2>/dev/null || true
    rm -rf "$DT_DIR"
else
    echo "  SKIP: python3 is not installed (the slow MCP endpoint needs it)"
fi
if [ $DT_OK -ne 1 ]; then echo "  detached-call smoke FAILED"; SMOKE_EXIT=1; fi

exit $SMOKE_EXIT
