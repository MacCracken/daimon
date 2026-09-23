# Security Audit — 2026-09-22: the agent lifecycle (2.3.0, with a 2.3.1 addendum)

2.3.0 connects process control to the HTTP API: an unauthenticated client can now start, stop,
pause, resume and delete agents. This audit covers that new surface and two older exposures that
process control made more serious. Every finding below was **observed**, not inferred: live probes
against the built binary (the output is quoted), tests that fail when the fix is reverted, and the
external references listed at the end.

## Attack surface added by 2.3.0

| Surface | Reached by | Exposure |
|---|---|---|
| `POST /v1/agents/{id}/start` | fork + execve of an operator-installed executable | API clients |
| `POST /v1/agents/{id}/stop` / `pause` / `resume` | SIGTERM → SIGKILL, SIGSTOP, SIGCONT to that process | API clients |
| `DELETE /v1/agents/{id}` | registry removal | API clients |
| the reap sweep | a non-blocking `waitpid` per agent, on every agent route | API clients |
| the child between fork and exec | descriptors, signal dispositions, rlimits, environment | the agent process |

---

## Findings

### VULN-011: The HTTP API listened on every interface (HIGH) — FIXED in 2.3.0

**CWE**: [CWE-1327](https://cwe.mitre.org/data/definitions/1327.html) — Binding to an Unrestricted IP Address.
**Reference**: [CVE-2023-48022](https://nvd.nist.gov/vuln/detail/CVE-2023-48022) ("ShadowRay") — Ray's job
submission API, unauthenticated by design and exposed to networks, was exploited in the wild to run
code on the cluster.

**Observed** (the 2.3.0 build before the fix, `serve 18095`):

```
$ ss -ltn | grep :18095
0.0.0.0:18095
```

**Cause**: both serve modes passed `INADDR_ANY()` (`lib/net.cyr`: `return 0`) to sandhi.
`config_new` has always set `listen_addr` to `"127.0.0.1"`, and nothing read it. The Rust original
bound `config.listen_addr:config.port` (`src/api.rs` at 0.6.0), so the port quietly widened an
unauthenticated API from loopback to the network.

**Fix**: `server_bind_addr()` (`src/server.cyr`) binds `listen_addr`. `serve --listen <IPv4>` sets it,
and `0.0.0.0` must now be asked for. A malformed value exits 1. The banner names the address. On
AGNOS the address is ignored: its kernel binds the single NIC (`sock_bind`, `lib/net.cyr`).

**Migration**: clients on other hosts (edge nodes, federation peers) need `serve --listen 0.0.0.0`,
behind a firewall. The API has no authentication.

**Tests**: `tests/smoke.sh` "bind address" reads `/proc/net/tcp`: 127.0.0.1 by default, 0.0.0.0 only
with `--listen 0.0.0.0`, exit 1 for a malformed address. `tests/config.tcyr` covers the setter.

---

### VULN-012: Cross-site requests and DNS rebinding reach the API (HIGH for agent control) — MITIGATED for agent control in 2.3.0; OPEN for the other routes

**CWE**: [CWE-352](https://cwe.mitre.org/data/definitions/352.html) (CSRF),
[CWE-346](https://cwe.mitre.org/data/definitions/346.html) (Origin Validation Error — the class recent
DNS-rebinding CVEs are filed under).
**References**: [CVE-2022-28108](https://nvd.nist.gov/vuln/detail/CVE-2022-28108) (Selenium Grid: CSRF
because it accepted `text/plain` and other non-JSON bodies; the payload started "an arbitrary binary
and arguments"), [CVE-2022-28109](https://nvd.nist.gov/vuln/detail/CVE-2022-28109) (Selenium Grid: DNS
rebinding), [CVE-2024-28224](https://nvd.nist.gov/vuln/detail/CVE-2024-28224) (Ollama: DNS rebinding
gave a web page the full local API). Selenium's fix rejected non-JSON content types and checked
`Origin` against an allowlist ([write-up](https://www.gabriel.urdhr.fr/2022/02/07/selenium-standalone-server-csrf-dns-rebinding-rce/)).

**Observed** (before the guard):

```
$ curl -X POST -H 'Content-Type: text/plain' -H 'Origin: https://attacker.example' .../v1/agents/1/start
{"id":1,"name":"csrf-target","type":"User","status":2,"pid":870915,"exit_code":null} [200]
$ curl -H 'Host: rebind.attacker.example:18095' .../v1/agents                  -> 200
$ curl -X OPTIONS -H 'Access-Control-Request-Method: DELETE' .../v1/agents/1   -> 405
```

A browser sends a `text/plain` POST from any page with no CORS preflight, so any web page an
operator opened could start and stop agents. daimon answers any `Host`, so a page on a
DNS-rebinding name reaches it as same-origin. Preflighted requests (DELETE, a JSON POST) were
already blocked, because the preflight `OPTIONS` gets a 405.

**Fix (agent control)**: `_route_refuse_browser` (`src/router.cyr`) answers **403** to any
start / stop / pause / resume / DELETE request that carries an `Origin` header, of any value
including `null` or empty. It records `agent.control.origin` on the audit chain. Browsers attach
`Origin` to cross-origin requests and a rebinding page sends its own, while daimon's clients
(agents, CLIs, curl) send none. `http_has_origin` relies on sandhi's header lookup, which matches
the whole name case-insensitively, so `X-Origin` and `Origins` do not count.

**Tests**: 8 in `tests/http.tcyr` (`http_origin`). 3 in `tests/smoke.sh`: a cross-site `text/plain` start
gets 403, nothing starts, and a DELETE carrying Origin gets 403.

**Still open** (roadmap 2.5.x):
- every other mutating route still accepts a cross-site simple POST: MCP tool registration, RAG
  ingest, edge decommission, scheduler cancel and submit;
- no `Host` allowlist, so a rebinding page can still READ every GET response;
- no authentication. The guard stops browsers, not a local process or a direct network client.

---

### VULN-013: Descriptors leaked into agent processes (HIGH) — FIXED before any route reached it

**CWE**: [CWE-403](https://cwe.mitre.org/data/definitions/403.html) — Exposure of File Descriptor to
Unintended Control Sphere.
**Reference**: [CVE-2024-21626](https://nvd.nist.gov/vuln/detail/CVE-2024-21626) ("Leaky Vessels") —
runc leaked an internal descriptor into the container process, which reached the host filesystem
through it.

**Observed**: `tests/smoke.sh`'s fixture agent counts the sockets it holds. With the close removed
from the child (a mutation run), it held **2**: daimon's listening socket and the connection of the
client that asked for the start. `lib/net.cyr`'s `tcp_socket` and `sock_accept` create sockets
without close-on-exec, so closing them in a forked child is the forking code's job, and daimon's
spawn did not do it.

**Impact had it shipped**: an agent could accept connections on daimon's port and answer as daimon.
It kept the port bound after daimon exited, so a restart could not listen, and it held the client's
connection open.

**Fix**: the child closes every descriptor from 3 up except its report pipe: `close_range` (Linux 5.9+),
falling back to one `close` per descriptor below the soft `RLIMIT_NOFILE`. stdin becomes
`/dev/null`, as in the Rust original (`Stdio::null()`).

---

### Design control: the executable is never taken from a request

The pre-2.3.0 `agent_start(h, executable)` carried a comment anticipating the route: "Once 2.3.x
wires this to a route, `executable` is a jget VIEW into the request buffer". Wired that way, an
unauthenticated API would run whatever binary a request named. That is the shape of CVE-2023-48022
and the payload of CVE-2022-28108. 2.3.0 does what the Rust original did (`find_agent_executable`):

- a registration names a **type**, System / User / Service (agnostik's `AgentType`);
- daimon runs `agnos-agent-<type>-agent` from `/usr/lib/agnos/agents` or `/opt/agnos/agents`, else
  `/usr/bin/agnos-agent-runner`;
- `serve --agents-dir DIR` is the operator's override. It is a CLI flag and never a request field.

The Rust original's third directory, `./agents`, is dropped. A search relative to the working
directory lets whoever controls daimon's cwd choose what it executes.

---

### VULN-014: A stop holds the single-threaded server (MEDIUM, denial of service) — OPEN

**CWE**: [CWE-400](https://cwe.mitre.org/data/definitions/400.html) — Uncontrolled Resource Consumption.

`agent_stop` waits up to `AGENT_STOP_GRACE_MS` (5000) for SIGTERM to work, then up to 1000 ms after
SIGKILL. The Rust original waited 10 s, but on an async runtime. daimon's handlers run to completion
in both serve modes, so every other request waits. Measured on 2.3.0, with an agent that ignores
SIGTERM:

```
sync:  stop took 5008 ms; a /v1/health sent 200 ms into it took 4805 ms
async: stop took 5009 ms; a /v1/health sent 200 ms into it took 4806 ms
```

(The stop took 5,359 ms before the grace moved from counted sleeps to a monotonic clock.) A client
that can reach the API and start such an agent type, if one is installed, can hold the server about
5 s per stop. The rate limit (120/min per IP) bounds that but does not prevent it.

**Remediation (roadmap)**: a non-blocking stop, answering 202 and escalating to SIGKILL from a
supervisor tick. That needs a periodic hook the sync server loop does not have. Authentication
(2.5.x) limits who can ask.

---

### VULN-015: Agents inherit daimon's environment (LOW) — ACCEPTED (Rust parity), recorded

**CWE**: [CWE-526](https://cwe.mitre.org/data/definitions/526.html) — Cleartext Storage of Sensitive
Information in an Environment Variable.

Every agent receives daimon's own environment, `agent_environ()` read from `/proc/self/environ`, as
tokio's `Command` gave it in the Rust original. Any secret placed in daimon's environment reaches
every agent. Before 2.3.0 the (never-run) spawn passed an EMPTY environment, which would have left
agents without `PATH`. **Remediation option**: an allowlisted environment per agent type.

---

### VULN-016: Any client can report any task's state (MEDIUM, integrity) — OPEN, recorded (2.3.1)

**CWE**: [CWE-862](https://cwe.mitre.org/data/definitions/862.html) — Missing Authorization.
**Reference (class)**: [CVE-2026-48592](https://basefortify.eu/cve_reports/2026/05/cve-2026-48592.html) —
Oban Web's job-edit handler skipped the authorization check that its sibling handlers made, so a
read-only user could substitute the worker a job would run.

2.3.1 adds `POST /v1/scheduler/tasks/{id}/start` and `/complete`, which report a task running and
done. There is no executor identity, so any caller that can reach the API can:
- mark any RUNNING task COMPLETED or FAILED. That returns its node's capacity while the real work
  may still be running, so the scheduler can over-commit the node;
- mark any SCHEDULED task RUNNING although nothing runs it. It then holds its capacity until
  something completes or cancels it.

**Mitigated by**: the loopback bind (VULN-011), and the Origin guard, which 2.3.1 applies to both
routes: a cross-site request gets 403 and is recorded as `task.control.origin`. Measured with
`tests/smoke.sh` ("a browser-originated complete is 403").
**Not mitigated**: a local process, or a network client when `--listen` widens the bind.
**Remediation**: per-agent identity (roadmap 2.5.x), so only a task's executor may report it.

`fail_reason` is stored from the request body. It is bounded by the 64 KB request limit, escaped on
output (`json_escape_str`), and cloned at the retention boundary, so a reused request buffer cannot
rewrite it (`tests/sched.tcyr`). The new `GET /v1/scheduler/nodes/{id}/tasks` is readable the way
every GET is, and so falls under VULN-012's open "no Host allowlist" item.

## Defects fixed in the lifecycle code when it first ran

None of this code had ever executed: it had no caller until 2.3.0. Each fix has a test that fails
when the fix is reverted. There were nine mutation runs against `tests/agent.tcyr`, and all nine
were caught.

| Defect | Consequence | Fix |
|---|---|---|
| `agent_stop` checked ONCE, with a non-blocking wait, right after SIGTERM | every agent was SIGKILLed; none shut down cleanly | a grace period on a monotonic clock |
| a paused (SIGSTOPped) agent was sent only SIGTERM | it cannot act on it; it would always be SIGKILLed | SIGCONT follows SIGTERM |
| the SIGPIPE ignore daimon sets survived `execve` | agents started with SIGPIPE ignored | `signal_default(SIGPIPE)` in the child (the stdlib's documented remedy) |
| a failed `execve` returned `Ok(pid)` | a missing executable looked like a running agent until exit 127 | a close-on-exec report pipe (what Rust's `Command::spawn` uses) |
| an rlimit that failed to apply also returned `Ok(pid)` | the start "succeeded" and the child exited 126 | `Err(AGENT_OP_NO_LIMITS)`, synchronously |
| `waitpid(0)` / `kill(0)` reachable through a pid of 0 | an agent record with no process would reap ANY child, or signal daimon's own process group — daimon included | `daimon_reap_code` / `daimon_signal` refuse pid ≤ 0 |
| exited agents were never reaped | they stayed RUNNING with their old pid (the kernel may reuse it), and zombies | `agent_reap` on every agent route |
| `config` `max_agents` (1000) was never enforced | registration was unbounded and permanent (no delete), and with 2.3.0 every record can become a process ([CWE-770](https://cwe.mitre.org/data/definitions/770.html)) | 409 at the limit; `DELETE` added |

## Recorded on the roadmap, not fixed here

- VULN-012's remainder and VULN-014, above.
- An agent's own children are not signalled: stop signals the agent's process, not a process group.
- Agents outlive a daimon crash: there is no parent-death signal, and the registry is in memory.
- stdout and stderr are inherited, not captured. The supervisor's `OutputCapture` is not wired.

## Verification limits

- **aarch64**: the cross-built binary was run under `qemu-aarch64` (user mode). Spawn, descriptors,
  stdin, SIGPIPE, stop, reap, the Origin guard and the loopback bind all behaved as on x86_64.
  RLIMIT_CPU was applied, but **RLIMIT_AS read `unlimited`**. QEMU's `linux-user/syscall.c`
  (`TARGET_NR_prlimit64`) passes a new limit through only when the resource is not RLIMIT_AS,
  RLIMIT_DATA or RLIMIT_STACK, and reports success for those three without applying them. So the
  emulator cannot show whether daimon's RLIMIT_AS call works on aarch64. That was not verified on
  aarch64 hardware.
- **AGNOS**: the build compiles. A start answers 501 until 2.4.x maps `sys_spawn_path`.

## Sources

- [CVE-2024-21626 — NVD](https://nvd.nist.gov/vuln/detail/CVE-2024-21626); [runc advisory GHSA-xr7r-f8xq-vfvv](https://github.com/opencontainers/runc/security/advisories/GHSA-xr7r-f8xq-vfvv)
- [CVE-2023-48022 — NVD](https://nvd.nist.gov/vuln/detail/CVE-2023-48022); [Oligo: ShadowRay](https://www.oligo.security/blog/shadowray-attack-ai-workloads-actively-exploited-in-the-wild)
- [CVE-2024-28224 — NCC Group advisory](https://www.nccgroup.com/research/technical-advisory-ollama-dns-rebinding-attack-cve-2024-28224/)
- [CVE-2022-28108 / CVE-2022-28109 — CSRF and DNS-rebinding to RCE in Selenium Server (Grid)](https://www.gabriel.urdhr.fr/2022/02/07/selenium-standalone-server-csrf-dns-rebinding-rce/)
- [CWE-403](https://cwe.mitre.org/data/definitions/403.html), [CWE-1327](https://cwe.mitre.org/data/definitions/1327.html), [CWE-346](https://cwe.mitre.org/data/definitions/346.html), [CWE-352](https://cwe.mitre.org/data/definitions/352.html), [CWE-400](https://cwe.mitre.org/data/definitions/400.html), [CWE-526](https://cwe.mitre.org/data/definitions/526.html), [CWE-770](https://cwe.mitre.org/data/definitions/770.html)
- [QEMU `linux-user/syscall.c`](https://gitlab.com/qemu-project/qemu/-/blob/master/linux-user/syscall.c) — the `TARGET_NR_prlimit64` handler
- [CVE-2026-48592 — Oban Web missing authorization](https://basefortify.eu/cve_reports/2026/05/cve-2026-48592.html); [CWE-862](https://cwe.mitre.org/data/definitions/862.html) (2.3.1 addendum)
