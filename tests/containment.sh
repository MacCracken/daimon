#!/bin/sh
# daimon 2.4.2: agent containment, a cgroup per agent. Run by tests/smoke.sh,
# inside a cgroup daimon may divide: this shell's own when it is writable (CI
# delegates one), else one scope of the user's systemd (systemd-run --user
# --scope), so that every daimon here shares it. Exits 0 when every check passed.
#
# Through 2.4.1 a stop reached the agent's process group, and a process that
# calls setsid leaves it: an agent's `setsid sleep` outlived its stop.
cd "$(dirname "$0")/.."
field() { grep -o "\"$1\":[^,}]*" | head -1 | cut -d: -f2 | tr -d '"'; }
CT_PORT=18094
CT_OK=1
CT_DIR=$(mktemp -d)
cat > "$CT_DIR/agnos-agent-user-agent" <<'AGENT'
#!/bin/sh
setsid sleep 30071 &
trap 'exit 0' TERM
: > "$0.ready"
while :; do sleep 0.05; done
AGENT
# Exits at once, leaving a process that ignores SIGTERM.
cat > "$CT_DIR/agnos-agent-system-agent" <<'AGENT'
#!/bin/sh
setsid sh -c 'trap "" TERM; while :; do sleep 0.1; done; : 30072' &
: > "$0.ready"
exit 0
AGENT
# Leaves a process that ignores SIGTERM, then waits for its own stop.
cat > "$CT_DIR/agnos-agent-service-agent" <<'AGENT'
#!/bin/sh
setsid sh -c 'trap "" TERM; while :; do sleep 0.1; done; : 30073' &
trap 'exit 0' TERM
: > "$0.ready"
while :; do sleep 0.05; done
AGENT
chmod +x "$CT_DIR"/agnos-agent-*
# The command lines of the two processes that ignore SIGTERM, as /proc shows them.
CT_LEFT2='sh -c trap "" TERM; while :; do sleep 0.1; done; : 30072'
CT_LEFT3='sh -c trap "" TERM; while :; do sleep 0.1; done; : 30073'
ct_check() {
    printf "  %s: " "$1"
    if [ "$2" = "$3" ]; then echo "PASS"; else echo "FAIL (got '$2', want '$3')"; CT_OK=0; fi
}
# Processes whose whole command line is $1 (or `setsid $1`, before setsid
# execs). Matched exactly, from /proc: a substring match, or pgrep -f, also
# counts any shell whose own command line holds the text (the one that runs
# this script, say).
ct_pids() {
    for c in /proc/[0-9]*/cmdline; do
        a=$({ tr '\0' ' ' < "$c"; } 2>/dev/null)
        a=${a% }
        if [ "$a" = "$1" ] || [ "$a" = "setsid $1" ]; then d=${c%/cmdline}; echo "${d#/proc/}"; fi
    done
}
ct_count() { ct_pids "$1" | wc -l | tr -d ' '; }
# Wait up to $2 tenths of a second for no process to match $1.
ct_gone() {
    i=0
    while [ $i -lt "$2" ] && [ "$(ct_count "$1")" != "0" ]; do i=$((i + 1)); sleep 0.1; done
    ct_count "$1"
}
ct_start_daimon() {
    ./build/daimon serve $CT_PORT --agents-dir "$CT_DIR" > "$CT_DIR/log.$1" 2>&1 &
    CT_SRV=$!
    i=0
    while [ $i -lt 30 ]; do
        if curl -s --max-time 1 "http://127.0.0.1:$CT_PORT/v1/health" >/dev/null 2>&1; then break; fi
        i=$((i + 1)); sleep 0.1
    done
}
C="http://127.0.0.1:$CT_PORT"
ct_start_daimon a
curl -s -o /dev/null --max-time 10 -X POST "$C/v1/agents" -d '{"name":"ct-user","type":"User"}'
CT_START=$(curl -s --max-time 10 -X POST "$C/v1/agents/1/start")
if [ "$(printf '%s' "$CT_START" | field contained)" = "true" ]; then
    CT_CG="/sys/fs/cgroup$(sed -n 's/^0:://p' /proc/$CT_SRV/cgroup)/daimon-$CT_SRV"
    ct_check "a start runs the agent in its own cgroup (contained)" "$(printf '%s' "$CT_START" | field contained)" "true"
    i=0; while [ $i -lt 50 ] && [ ! -e "$CT_DIR/agnos-agent-user-agent.ready" ]; do i=$((i + 1)); sleep 0.1; done
    ct_check "... where the process it started with setsid runs" "$(ct_count "sleep 30071")" "1"
    CT_STOP=$(curl -s --max-time 15 -X POST "$C/v1/agents/1/stop")
    ct_check "a stop ends the agent (status 5)" "$(printf '%s' "$CT_STOP" | field status)" "5"
    ct_check "... and the process that left its group (it outlived the stop through 2.4.1)" \
      "$(ct_gone "sleep 30071" 30)" "0"
    # An agent that exits leaving a process that ignores SIGTERM: SIGTERM, the 5 s
    # grace, then cgroup.kill.
    curl -s -o /dev/null --max-time 10 -X POST "$C/v1/agents" -d '{"name":"ct-sys","type":"System"}'
    curl -s -o /dev/null --max-time 10 -X POST "$C/v1/agents/2/start"
    i=0; while [ $i -lt 50 ] && [ ! -e "$CT_DIR/agnos-agent-system-agent.ready" ]; do i=$((i + 1)); sleep 0.1; done
    ct_check "an agent that exits leaves a process that ignores SIGTERM" "$(ct_count "$CT_LEFT2")" "1"
    ct_check "... which is ended once the grace period is over" "$(ct_gone "$CT_LEFT2" 90)" "0"
    i=0; while [ $i -lt 30 ] && [ -n "$(ls -d "$CT_CG"/agent-* 2>/dev/null)" ]; do i=$((i + 1)); sleep 0.1; done
    ct_check "... and the agents' cgroups are removed" "$(ls -d "$CT_CG"/agent-* 2>/dev/null | wc -l | tr -d ' ')" "0"
    # daimon killed outright: its agent dies with it (PDEATHSIG), but what the
    # agent left in its cgroup does not. The next daimon in this cgroup ends it.
    curl -s -o /dev/null --max-time 10 -X POST "$C/v1/agents" -d '{"name":"ct-svc","type":"Service"}'
    curl -s -o /dev/null --max-time 10 -X POST "$C/v1/agents/3/start"
    i=0; while [ $i -lt 50 ] && [ ! -e "$CT_DIR/agnos-agent-service-agent.ready" ]; do i=$((i + 1)); sleep 0.1; done
    kill -9 $CT_SRV; wait $CT_SRV 2>/dev/null
    sleep 0.3
    ct_check "what an agent left outlives a daimon killed outright" "$(ct_count "$CT_LEFT3")" "1"
    ct_start_daimon b
    ct_check "... and the next daimon started in its cgroup ends it" "$(ct_gone "$CT_LEFT3" 30)" "0"
    i=0; while [ $i -lt 30 ] && [ -d "$CT_CG" ]; do i=$((i + 1)); sleep 0.1; done
    ct_check "... and removes the dead daimon's cgroups" "$([ -d "$CT_CG" ] && echo left || echo gone)" "gone"
else
    ct_check "a start reports whether the agent is contained" "$(printf '%s' "$CT_START" | field contained)" "false"
    echo "  SKIP: daimon cannot make cgroups here: its cgroup is not delegated to it"
    curl -s -o /dev/null --max-time 15 -X POST "$C/v1/agents/1/stop"
    for p in $(ct_pids "sleep 30071"); do kill "$p" 2>/dev/null; done
fi
kill $CT_SRV 2>/dev/null; wait $CT_SRV 2>/dev/null
rm -rf "$CT_DIR"
[ $CT_OK -eq 1 ]
