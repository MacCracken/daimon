#!/bin/sh
# daimon's aarch64 VM smoke: run by tests/aarch64/run.sh inside its VM, as nobody,
# after the suites. The suites exercise daimon's modules; this exercises the
# linked aarch64 binary on a real aarch64 kernel: its event loop and sockets, an
# agent's start, its channel frame, its stop and exit status, its cgroup, and the
# Host check. The VM has busybox and no curl, so requests go through nc.
#
#   sh smoke.sh <daimon binary>      # prints "SMOKE n passed, m failed"; exits m
BIN=$1
PORT=18090
D=$(mktemp -d)
OK=0
BAD=0
check() {
    if [ "$2" = "$3" ]; then OK=$((OK + 1)); echo "  $1: PASS"
    else BAD=$((BAD + 1)); echo "  $1: FAIL (got '$2', want '$3')"; fi
}
# req METHOD PATH [BODY] [HOST]: RESP is the whole answer, CODE its status.
req() {
    b=${3:-}
    h=${4:-127.0.0.1}
    RESP=$(printf '%s %s HTTP/1.1\r\nHost: %s\r\nContent-Length: %d\r\nConnection: close\r\n\r\n%s' \
        "$1" "$2" "$h" "${#b}" "$b" | nc -w 10 127.0.0.1 $PORT 2>/dev/null)
    CODE=$(printf '%s' "$RESP" | head -n 1 | cut -d' ' -f2)
}
has() { if printf '%s' "$RESP" | grep -q -- "$1"; then echo yes; else echo no; fi; }

# The user agent sends one frame on its channel (fd 3), records the ACK byte,
# and exits 0 on SIGTERM. The system agent exits 3 at once.
cat > "$D/agnos-agent-user-agent" <<'AGENT'
#!/bin/sh
trap 'exit 0' TERM
F='{"target":"*","type":"event","payload":{"greeting":"hello from aarch64","n":1}}'
printf "\\000\\000\\000\\$(printf %03o ${#F})%s" "$F" >&3
head -c 1 <&3 | od -An -tu1 | tr -d ' ' > "$0.ack"
: > "$0.ready"
while :; do sleep 0.05; done
AGENT
printf '#!/bin/sh\nexit 3\n' > "$D/agnos-agent-system-agent"
chmod +x "$D"/agnos-agent-*

"$BIN" serve $PORT --agents-dir "$D" > "$D/daimon.log" 2>&1 &
SRV=$!
i=0
while [ $i -lt 50 ]; do
    req GET /v1/health
    [ "$CODE" = "200" ] && break
    i=$((i + 1)); sleep 0.1
done
check "the aarch64 binary answers health" "$CODE" "200"
if [ "$CODE" != "200" ]; then echo "  daimon's log:"; sed 's/^/    /' "$D/daimon.log"; fi
req POST /v1/agents '{"name":"vm-user"}'
check "an agent registers" "$CODE" "201"
req POST /v1/agents/1/start
check "it starts" "$CODE" "200"
check "... RUNNING" "$(has '"status":2')" "yes"
check "... under its limits" "$(has '"limits_enforced":true')" "yes"
check "... in a cgroup of its own" "$(has '"contained":true')" "yes"
i=0
while [ $i -lt 50 ] && [ ! -e "$D/agnos-agent-user-agent.ready" ]; do i=$((i + 1)); sleep 0.1; done
check "its frame on fd 3 is ACKed (1)" "$(cat "$D/agnos-agent-user-agent.ack" 2>/dev/null)" "1"
req POST /v1/agents/1/messages/take
check "an HTTP client takes it" "$CODE" "200"
check "... from the agent, with its payload" "$(has '"greeting":"hello from aarch64"')" "yes"
req POST /v1/agents/1/stop
check "a stop answers" "$CODE" "200"
check "... STOPPED" "$(has '"status":5')" "yes"
check "... with the agent's own exit code" "$(has '"exit_code":0')" "yes"
req POST /v1/agents '{"name":"vm-sys","type":"System"}'
check "a system agent registers" "$CODE" "201"
req POST /v1/agents/2/start
check "... starts" "$CODE" "200"
i=0
while [ $i -lt 50 ]; do
    req GET /v1/agents/2
    [ "$(has '"status":6')" = "yes" ] && break
    i=$((i + 1)); sleep 0.1
done
check "... FAILED once it exits" "$(has '"status":6')" "yes"
check "... with exit code 3" "$(has '"exit_code":3')" "yes"
req GET /v1/health "" attacker.example
check "a request naming another host is refused" "$CODE" "403"
req GET /v1/metrics
check "metrics answer" "$CODE" "200"
check "... with the slot counter" "$(has '"http_evicted":0')" "yes"
req DELETE /v1/agents/1
check "an agent is deleted" "$CODE" "200"
kill $SRV 2>/dev/null
wait $SRV 2>/dev/null
echo "SMOKE $OK passed, $BAD failed"
exit $BAD
