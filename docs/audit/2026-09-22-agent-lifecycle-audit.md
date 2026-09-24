# Security Audit — 2026-09-22: the agent lifecycle (2.3.0, with 2.3.1, 2.3.2, 2.3.3, 2.3.4 and 2.4.2 addenda)

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
| the agent's channel, fd 3 (2.3.3) | length-prefixed JSON frames, read by a service thread | the agent process |

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

### VULN-012: Cross-site requests and DNS rebinding reach the API (HIGH for agent control) — MITIGATED for agent control in 2.3.0; FIXED for the other routes in 2.3.4

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

**Fixed in 2.3.4** (`http_host_allowed` / `http_origin_foreign`, `src/http.cyr`; checked in
`handle_request`):
- **Host allowlist.** While daimon listens on loopback, a request must name a loopback host
  (`127.0.0.0/8`, `localhost`, `[::1]`). Anything else gets 403 (`http.host.reject` on the audit
  chain), so a DNS-rebinding page can neither write nor READ. With `--listen` beyond loopback the
  operator has chosen daimon's names, and the check is off. Reference: [CVE-2024-28224](https://www.nccgroup.com/research/technical-advisory-ollama-dns-rebinding-attack-cve-2024-28224/)
  (Ollama), fixed the same way.
- **Cross-site writes on every route.** POST, PUT and DELETE carrying an `Origin` from another site
  get 403 (`http.origin.reject`): MCP tool registration, RAG ingest, edge decommission, scheduler
  cancel and submit, messages. An origin on a loopback host may still write, so a local browser UI
  keeps working. Agent and task control still refuse any `Origin`.
- Tests: 22 in `tests/http.tcyr` (`http_host`); 6 smoke checks.

**Still open**: no authentication (roadmap 2.5.x). These checks stop browsers, not a local process or
a direct network client.

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

### VULN-014: A stop holds the single-threaded server (MEDIUM, denial of service) — FIXED in 2.3.4

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

**Fixed in 2.3.4**: daimon now runs its own event loop (ADR-006), so it has the periodic tick the
sync loop lacked. `agent_stop_begin` signals the agent and returns; the loop's upkeep sends SIGKILL
at the deadline and collects the process (`agent_stop_step`). The stop route **defers its answer**
(`server_defer`): the client is answered when the agent is gone, with the same body as before, and
everyone else is served meanwhile. Measured on 2.3.4 with an agent that ignores SIGTERM:

```
the stop answered after 5.01 s (status 5, exit_code 137)
/v1/health during it, five times 0.5 s apart: 0.001 s each
```

Smoke checks it on every run.

---

### VULN-015: Agents inherit daimon's environment (LOW) — ACCEPTED as the default (Rust parity); an operator option since 2.3.4

**CWE**: [CWE-526](https://cwe.mitre.org/data/definitions/526.html) — Cleartext Storage of Sensitive
Information in an Environment Variable.

Every agent receives daimon's own environment, `agent_environ()` read from `/proc/self/environ`, as
tokio's `Command` gave it in the Rust original. Any secret placed in daimon's environment reaches
every agent. Before 2.3.0 the (never-run) spawn passed an EMPTY environment, which would have left
agents without `PATH`.

**2.3.4**: `serve --agent-env minimal` gives agents only PATH, HOME, USER, LOGNAME, SHELL, TERM,
TMPDIR, TZ, LANG, LANGUAGE, LC_*, XDG_RUNTIME_DIR and AGNOS_*, keeping daimon's other variables
(tokens, credentials) out of them. The default stays `inherit`: changing what every agent receives
would break agents that rely on it, which is for a major version.

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

### VULN-017: Request bodies were read by a flat, non-decoding parser — a parser differential (MEDIUM) — FIXED in 2.3.2

**CWE**: [CWE-436](https://cwe.mitre.org/data/definitions/436.html) — Interpretation Conflict.
**References**:
- [CVE-2017-12635](https://docs.couchdb.org/en/stable/cve/2017-12635.html) — CouchDB's Erlang JSON
  parser took the FIRST duplicate key and its JavaScript parser the LAST, so a user could grant
  themselves `_admin`.
- Bishop Fox, [An Exploration & Remediation of JSON Interoperability Vulnerabilities](https://bishopfox.com/blog/json-interoperability-vulnerabilities).
  It recommends a fatal error on duplicate keys, and warns against truncating characters that
  different parsers may treat differently.

Until 2.3.2 almost every handler read its body with `json_parse` + `jget`, bayan's FLAT parser.
bayan's own contract, above `bayan_json_parse`, says its values are the raw source bytes with
escapes NOT decoded, and that a caller who needs decoded values should use the tagged-tree parser.
Its scan also ends an unquoted value at the first `,` or `}`. **Observed**: this release's smoke
section was run against the committed 2.3.1 code, and all ten checks failed:

```
{"name":"say \"hi\" \\o/ \u00e9"}                  -> stored `say \"hi\" \\o/ \u00e9`, escapes intact
{"meta":{"x":1,"name":"nested"},"name":"top"}      -> name = nested
{"meta":{"x":1},"name":"after"}                    -> name absent ("unnamed")
POST /v1/mcp/call {"arguments":{"x":1,"name":"libro_export"},"name":"libro_verify"}
                                                   -> ran libro_export
{"callback_url":"http:\/\/127.0.0.1:9\/"}          -> 400: the SSRF guard saw `http:\/\/`
name=formish · {"name":"a","name":"b"} · {"name":"a\u0000b"}   -> all 201
a control byte in stored text                     -> dropped on output (json_escape_str)
```

**Impact**: the MCP line is the differential. daimon ran a different tool from the one the body's
top-level `name` names, and the body it forwards, and any JSON-aware proxy or policy check in
front of daimon, say the other. Nothing else in daimon was escalated by it, since the caller chooses
both names. But it defeats any rule enforced upstream on the tool name. The escaped-slash URL was a
false refusal, not a bypass: nothing decoded the stored raw URL later.

**Fix**: `http_body_json` and the `http_json_str` / `_int` / `_has` / `_text` readers
(`src/http.cyr`):
- the typed parser, which decodes every escape and keeps nesting;
- a body that is not one JSON object, that has a duplicate top-level key (the CVE-2017-12635
  shape), or that has U+0000 in a top-level string, is refused with 400. U+0000 would be truncated
  wherever the field becomes a C string: registry keys, argv, audit entries;
- decoded strings are fresh copies, never views into the request buffer;
- `json_escape_str` writes every other control byte as `\u00XX` instead of dropping it;
- `jget` / `jget_int` are removed, so the flat read is no longer on hand.

**Tests**: 34 new assertions in `tests/http.tcyr`. A new fuzz harness, `fuzz/json_strings.fcyr`
(16,000 cases), checks encode → read and reference-encode → read round trips, the U+0000 and
duplicate-key refusals, and that nested keys never surface. There are 10 smoke checks, which pass
on 2.3.2 and all fail on 2.3.1. Five mutation runs were all caught.

### VULN-018: An agent's messages grow daimon's heap without bound (MEDIUM, denial of service) — FIXED in 2.3.4

**CWE**: [CWE-770](https://cwe.mitre.org/data/definitions/770.html) — Allocation of Resources Without
Limits or Throttling; [CWE-400](https://cwe.mitre.org/data/definitions/400.html) — Uncontrolled
Resource Consumption.
**Reference class**: [CVE-2018-16865](https://www.cve.org/CVERecord?id=CVE-2018-16865). In
systemd-journald, "an allocation of memory without limits" when many entries are sent to the
journal socket.

daimon's heap is a bump allocator that never frees. **Measured** on 2.3.3 (a 100-byte frame with a
nested payload, 90 frames):
- **451 bytes** stay allocated per message delivered to the bus: 224 in the service thread (the
  record and the target and payload copies) and 227 in `ipc_drain` (the bus message, its copies,
  the queue push);
- **0 bytes** per frame refused, including those refused because the hand-off was full.

Messages are delivered only when an HTTP request drains the hand-off (at most 1024 per request),
and one agent's channel ran ~86,000 round trips a second in the benchmark (`ipc_frame_roundtrip`,
11.6 µs). An agent that also sends HTTP requests (loopback, unauthenticated) can therefore grow
daimon's memory for as long as it likes, on the order of tens of MB a second. The same is true of
HTTP requests alone, whose allocations are never freed either. This finding puts the old
exposure in the hands of every started agent.

**Mitigated by**:
- the bounded queues (100 per agent) and hand-off (1024);
- refused frames costing nothing (fixed during 2.3.3, below);
- the parse running in a per-frame arena (the tree cost 1,496 bytes a frame on the shared heap).

**Fixed in 2.3.4**: a message is now **one freelist block** (`lib/freelist.cyr`, which frees), held
by every queue it is on and freed when the last lets go — taken (`POST .../messages/take`), or its
agent deleted. The bus holds at most **64 MiB** (`IPC_BUS_BYTES_MAX`); past it, and past a full
queue, nothing is queued, and a channel's sender is told (`NACK_QUEUE_FULL`). So the bus's memory is
bounded by what is queued, not by what was ever sent. The frame's parse stays in the channel
arena; its payload is built there too, so a delivered frame no longer allocates on the bump heap
at all. Measured: 1000 messages published and taken through one queue leave the bump heap as it
was (under 1 KiB of change, from the queue vector), and `bus_bytes` returns to 0. Tests:
`tests/ipc.tcyr` (`ipc_bus_memory`), including that a freed message's block is reused by the next.

What remains is the general property that daimon's HTTP handlers allocate per request on the bump
heap, which never frees: the 3.0.0 arena-isolation arc.

## 2.3.3 addendum — agent channels

2.3.3 gives each started agent a **channel**. It is a socketpair: the agent's end is its fd 3
(`AGNOS_IPC_FD=3`), and a service thread in daimon reads daimon's end. The design is recorded in
[ADR-005](../adr/005-agent-channels.md) and the wire protocol in
[docs/guides/agent-ipc.md](../guides/agent-ipc.md). The socket-FILE endpoint the port carried
(`agent_ipc_new` / `bind` / `accept_one` / `send` / `cleanup`) is **removed**. Nothing called it.

### The three parked defects, resolved by removal

| Defect (found 2.2.3) | CWE | Now |
|---|---|---|
| the `SO_PEERCRED` check **failed open**: a failed `getsockopt` skipped the check | [CWE-636](https://cwe.mitre.org/data/definitions/636.html) Not Failing Securely | there is no peer to check. Only the agent's process received the other end of its pair. Agents all run as daimon's uid, so a uid check could never have told them apart |
| a message cut short by its sender was queued and ACKed as whole | [CWE-130](https://cwe.mitre.org/data/definitions/130.html) Improper Handling of Length Parameter Inconsistency | a frame is handed over only when all `len` bytes have arrived. EOF mid-frame closes the channel and is audited (`ipc.frame.short`) |
| `accept_one` read with no timeout, so a silent peer held the accept loop | [CWE-400](https://cwe.mitre.org/data/definitions/400.html) | reads never block. A begun frame has 5 s to finish, then the channel closes (`ipc.frame.timeout`) |

### The new surface, against known IPC-daemon failures

| Class | Reference | Control in 2.3.3 |
|---|---|---|
| incomplete messages pin a daemon's resources | [CVE-2014-3639](https://www.cve.org/CVERecord?id=CVE-2014-3639): dbus-daemon "does not properly close old connections … via a large number of incomplete connections" | one channel per agent, no accept. A begun frame times out after 5 s and closes its channel |
| one client's volume starves the rest | [CVE-2014-3638](https://www.cve.org/CVERecord?id=CVE-2014-3638): D-Bus, "denial of service (CPU consumption) via a large number of method calls" | at most 16 frames per channel per service pass; the rest wait for the next pass |
| socket input drives allocation | [CVE-2018-16865](https://www.cve.org/CVERecord?id=CVE-2018-16865) (journald) | frames capped at 64 KiB, allocated on the heap (never the stack). A refused frame allocates nothing. What remains is VULN-018 |
| spoofed identity | — | the source is the channel's agent; a frame's claim of any other source is ignored (tested) |
| parser differential | VULN-017 | frames pass the same `json_object_ok` as HTTP bodies: no duplicate top-level key, no U+0000 in a top-level string |

### Found and fixed during 2.3.3, before release

| Defect | Consequence, measured | Fix |
|---|---|---|
| daimon's epilogue called `sys_exit`, which is exit(2) and ends only the calling thread | once an agent had started, a `serve` that returned would leave daimon up with its main thread gone. The test suites showed it: the ipc suite printed its summary and then never exited | `sys_exit_group`, as `lib/syscalls.cyr` directs for a program epilogue |
| a frame refused because the hand-off was full had already built its record | 224 bytes leaked per refused frame, so an agent flooding a full hand-off grew daimon without bound | the room is checked before anything is allocated |
| a broadcast built a key vector per message (`map_keys`) | a never-freed allocation per broadcast, and a slower broadcast | `map_iter` over the subscribers: `bus_broadcast_100` 11.18 → 3.37 µs |

### Thread safety

The service thread touches:
- its channel list, body buffers and parse arena (all its own);
- two bounded thread channels (`lib/thread.cyr`, mutex-protected);
- three counters, read and written atomically across threads;
- `alloc()`, which takes its lock from the moment the thread starts.

It never touches the bus, the agent registry, the audit chain or the logger; the main thread
applies what it hands over. The parse is bayan's reentrant `bayan_json_v_parse_ctx_a`, into the
thread's own arena. **Fork**: the main thread forks agents while the service thread may hold the
allocator lock. The child path (`_agent_child`) uses only syscalls, stack buffers and string
literals until `execve`, so it never takes a lock copied in a held state.

### Verification

- **Tests**:
  - `tests/ipc.tcyr`: 111 assertions, through the real service thread. They cover ACK, every
    refusal, keep-alive, split and joined frames, exactly 65536 bytes, oversize, short, timeout, a
    full hand-off, drain, audit, remove-after-read, replace, and the arena's heap cost.
  - `tests/agent.tcyr`: 138 assertions. An agent sees `AGNOS_IPC_FD=3` and a socket on fd 3, fds 4–9
    are closed, its frame is ACKed, and daimon's end closes on stop and on reap even while the
    agent's child still holds fd 3.
- **12 mutation runs, all caught.**
- **14 new HTTP smoke checks.**
- **aarch64** (qemu-aarch64): `ipc.tcyr` 111/111. `agent.tcyr` shows the same six failures as 2.3.2
  does under qemu: QEMU applies no RLIMIT_AS, and qemu-user adds a thread of its own.
- **AGNOS**: builds. A start answers 501 before any channel is made.

## 2.3.4 addendum — daimon's own event loop

2.3.4 replaces sandhi's serve loops with daimon's own single-threaded epoll loop (ADR-006) and, with
it, fixes VULN-014, VULN-018 and VULN-012's remainder, above. What the new loop changes about the
attack surface:

| Surface | 2.3.3 | 2.3.4 |
|---|---|---|
| a client that sends slowly (the Slowloris class, [CVE-2007-6750](https://www.cve.org/CVERecord?id=CVE-2007-6750)) | held the whole server: sandhi's blocking recv waited up to 5 s per read, and trickling reset it | holds one of 128 slots; closed after 5 s idle or 30 s without a whole request. Measured: a request beside a stalled client answered in 6 ms |
| a client that never reads its answer | could hold the server in `write` indefinitely (sandhi set no send timeout) | `SO_SNDTIMEO` of 5 s |
| request framing and smuggling | sandhi's `recv_request` and three checks | the same rule and the same three checks; 8 probes answered identically on both |
| agent channels | a second thread; every allocation took the heap lock | the same loop; no thread (`_threads_active` stays 0, tested) |

**Agents and their processes (2.3.4):**
- Each agent **leads its own process group** (`setpgid` in the child), and stop / pause / resume
  signal the group (`daimon_signal_tree`): a stop used to leave the agent's children running.
  **Residual**: a child that leaves the group (`setsid`, `setpgid`) is out of reach, as with any
  process-group supervisor. Only a cgroup would hold it (Linux-specific), and that is recorded on the
  roadmap.
- Each agent **dies with daimon** (`PR_SET_PDEATHSIG` = SIGKILL, with the check for a daimon that died
  before the prctl). The registry is in memory, so a survivor could never be reached. **Residual**: the
  agent's own children are not the daimon's, so they get no death signal; they lose the agent,
  and with it their fd 3's peer.
- **Descriptor limit**: each running agent holds one of daimon's descriptors (its channel), two with
  `--agent-output capture`, and the loop keeps up to 128 connections open. daimon raises its soft
  `RLIMIT_NOFILE` to fit `max_agents` (4192 for the default 1000) and gives each agent the limit it
  started with (1024 in the run below). It warns at start when the hard limit is lower. Measured
  with the limit pinned at 1024, systemd's default soft limit: in capture mode the 507th start
  answered 500 while the API kept answering. 2.3.3 (four descriptors of its own, one per agent) fit
  the default 1000 agents.
- **Captured output** (`--agent-output capture`, opt-in): a fixed ring per agent (32 KiB, one freelist
  block), so a chatty agent costs daimon that ring and no more; invalid UTF-8 is repaired before it
  becomes JSON (`json_escape_text`).

- **Calls to other servers are bounded and off the loop** ([CWE-1088](https://cwe.mitre.org/data/definitions/1088.html),
  synchronous access of a remote resource without a timeout). sandhi's HTTP client applies no
  timeout unless its caller sets one (read, write, connect and total all default to 0), and daimon
  set none. So an MCP endpoint that accepted and never answered held daimon: measured on 2.3.3,
  `/v1/health` got no answer 11 s into such a call, after its caller had given up. 2.3.4 runs
  forwarded MCP calls (`tools/call`, `resources/read`, `prompts/get`) and `web_fetch` /
  `web_search` in a child process (`server_detach`). The child keeps only the pipe it answers on,
  dies with daimon, and is killed at 60 s; its client is answered 504 and the event audited
  (`http.detached.timeout`). Measured: `/v1/health` during a 2 s call, 1.70 s → 0.0004–0.0005 s.

**Residual, recorded:**
- **An agent start runs on the loop**: its fork and exec report take milliseconds.
- **One local client can take all 128 slots** by holding 128 slow connections, each for up to 30 s.
  All loopback clients share 127.0.0.1, so a per-IP cap would be a global cap. It is bounded, and
  far from 2.3.3, where one slow connection held the server.

**Verification**: 25 mutation runs over the 2.3.4 changes, all caught (one only after a test was
added for it). aarch64 under qemu: the loop served health, an agent start, its channel's frame (the
aarch64 `epoll_event` layout), take, captured output, a stop, the Host check and a detached MCP call.

## 2.4.2 addendum — the three residuals

**A child that leaves its agent's process group** (the 2.3.4 residual above). Measured on 2.4.1: an
agent's `setsid sleep` was still running after its stop answered. 2.4.2 runs each agent in a cgroup
of its own wherever daimon's cgroup (cgroup v2) is its to divide ([ADR-008](../adr/008-agent-containment-cgroup.md)):
- a systemd service with `Delegate=yes`, or anything the user's systemd runs;
- a cgroup made for daimon's user, which CI does.

The agent moves itself in before exec. A stop signals everything in the cgroup, and SIGKILL is
`cgroup.kill`. When the agent's process is collected, what it left gets SIGTERM, the grace period and
`cgroup.kill`, and the cgroup is removed. The next daimon ends what a daimon that was killed outright
left.
- Measured with `tests/containment.sh`, 10 checks: the `setsid sleep` is gone after the stop. A
  leftover that ignores SIGTERM is gone once the grace is over. What an agent left when daimon was
  killed with SIGKILL is ended by the next daimon started in the same cgroup.
- The same checks passed as an ordinary user in a cgroup handed to that user, as CI does it,
  rehearsed in a privileged `ubuntu:24.04` container.
- **Found in review, fixed before release**:
  - The first version checked that an agent had entered its cgroup by reading the cgroup's
    `cgroup.procs`, which lists live processes only. An agent that exited before daimon looked was
    judged never to have entered. It was audited as a failure, and what it had started stayed in a
    cgroup daimon no longer tracked. The check now reads `/proc/<pid>/cgroup`, which names the
    cgroup until the process is collected. Measured on the kernel first: a zombie is absent from
    `cgroup.procs` and present in `/proc/<pid>/cgroup`.
  - A cgroup holding cgroups the agent made inside it could never be removed, and stayed queued.
    Once nothing runs in it or below it (`cgroup.events`), they are now removed, deepest first.
  - When an agent's process was collected, a cgroup holding only the empty cgroups it had made was
    audited as processes left behind (`agent.cgroup.leftover`), and waited out the grace period. It
    is now removed then.
  - daimon decided it could contain agents from a `mkdir` alone. Moving a process also needs write
    access to the `cgroup.procs` of the cgroup it leaves, which is now checked at startup.

  The removal allocates, and the loop's upkeep runs every 100 ms under a bump allocator with no
  free. So the queue tries it at most three times per cgroup, and a check measures that the upkeep
  then allocates nothing. Every fault above but the last has a check in `tests/agent.tcyr` that fails
  on the code before its fix. That was run: the lookup through `cgroup.procs` failed 1 check
  uncontained and 2 contained. The queue without the removal failed 2, the release without it 2,
  and the removal without its bound 1.
- **Residual**: where daimon's cgroup is not its own (a login session's scope is root's, cgroup v1),
  agents are not contained. That is warned and audited at startup (`agent.cgroup.unavailable`), and
  the agent's JSON says `"contained":false`. Containment is not a security boundary between an agent
  and daimon: they run as the same user, so an agent can write its own processes into another cgroup
  that user may write, and take them out of reach.

**One local client could take all 128 slots** (the 2.3.4 residual above). Measured on 2.4.1: with 128
connections each sending a byte every 2 s, a health check from a new client waited 28.6 s. In 2.4.2,
when every slot is taken and a connection is waiting, the oldest request still being read gives way
once it is 1 s old. It is closed unanswered and counted (`http_evicted`). Measured: the same health
check is answered in 1 ms, and one held connection gave way ([ADR-006's 2.4.2 addendum](../adr/006-own-event-loop.md)).
**Residual**: a client that floods new connections can keep every slot younger than a second and fill
the kernel's backlog, and so still delay others. Slow requests no longer hold slots for 30 s, but a
connection flood is not prevented (not measured).

**RLIMIT_AS on aarch64** (the verification limit below). `tests/aarch64/run.sh` boots a real aarch64
Linux kernel (Alpine 3.24.2, 6.18.52) under `qemu-system-aarch64`. It runs the agent and
syscall-portability suites in it as `nobody`, in a cgroup handed to `nobody`, so the containment
checks take the contained path. All 212 agent checks pass there, and all 46 portability checks,
including RLIMIT_AS applied to the child, and an address-space limit that cannot be applied refusing
the start. The same binary under user-mode `qemu-aarch64`: 206 of 212, with the six failures being
the three RLIMIT_AS checks and three thread counts. CI runs the VM test (the
`aarch64-vm` job). This is a VM, not hardware, but the rlimit code it exercises is the kernel's, which
is what the user-mode emulator replaced.

**Sources**: [Linux cgroup v2](https://docs.kernel.org/admin-guide/cgroup-v2.html) (delegation
containment, `cgroup.kill`, the no-internal-process rule); [systemd.kill(5)](https://www.freedesktop.org/software/systemd/man/latest/systemd.kill.html)
(`KillMode=control-group`); [CVE-2007-6750](https://www.cve.org/CVERecord?id=CVE-2007-6750) (Slowloris).

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

- (2.3.4 fixed VULN-012's remainder, VULN-014 and VULN-018, above, and the four items below.)
- ~~An agent's own children are not signalled~~ — process groups (2.3.4).
- ~~Agents outlive a daimon crash~~ — `PR_SET_PDEATHSIG` (2.3.4).
- ~~stdout and stderr are inherited, not captured~~ — `--agent-output capture` (2.3.4).
- ~~A child that leaves its agent's group is out of reach~~ — a cgroup per agent (2.4.2).
- ~~One local client can take all 128 slots~~ — the oldest trickling request gives way (2.4.2).

## Verification limits

- **aarch64**: the cross-built binary was run under `qemu-aarch64` (user mode). Spawn, descriptors,
  stdin, SIGPIPE, stop, reap, the Origin guard and the loopback bind all behaved as on x86_64.
  RLIMIT_CPU was applied, but **RLIMIT_AS read `unlimited`**. QEMU's `linux-user/syscall.c`
  (`TARGET_NR_prlimit64`) passes a new limit through only when the resource is not RLIMIT_AS,
  RLIMIT_DATA or RLIMIT_STACK, and reports success for those three without applying them. So the
  emulator cannot show whether daimon's RLIMIT_AS call works on aarch64. That was not verified on
  aarch64 hardware. *(2.4.2: verified under a real aarch64 kernel in a VM, `tests/aarch64/run.sh`,
  all 212 agent checks. See the 2.4.2 addendum.)*
- **AGNOS**: the build compiles. A start answers 501 until 2.4.x maps `sys_spawn_path`. *(2.4.0:
  agents start on AGNOS, tested on its kernel under QEMU. See ADR-007 and
  [the AGNOS audit](2026-09-23-agnos-platform-audit.md).)*

## Sources

- [CVE-2024-21626 — NVD](https://nvd.nist.gov/vuln/detail/CVE-2024-21626); [runc advisory GHSA-xr7r-f8xq-vfvv](https://github.com/opencontainers/runc/security/advisories/GHSA-xr7r-f8xq-vfvv)
- [CVE-2023-48022 — NVD](https://nvd.nist.gov/vuln/detail/CVE-2023-48022); [Oligo: ShadowRay](https://www.oligo.security/blog/shadowray-attack-ai-workloads-actively-exploited-in-the-wild)
- [CVE-2024-28224 — NCC Group advisory](https://www.nccgroup.com/research/technical-advisory-ollama-dns-rebinding-attack-cve-2024-28224/)
- [CVE-2022-28108 / CVE-2022-28109 — CSRF and DNS-rebinding to RCE in Selenium Server (Grid)](https://www.gabriel.urdhr.fr/2022/02/07/selenium-standalone-server-csrf-dns-rebinding-rce/)
- [CWE-403](https://cwe.mitre.org/data/definitions/403.html), [CWE-1327](https://cwe.mitre.org/data/definitions/1327.html), [CWE-346](https://cwe.mitre.org/data/definitions/346.html), [CWE-352](https://cwe.mitre.org/data/definitions/352.html), [CWE-400](https://cwe.mitre.org/data/definitions/400.html), [CWE-526](https://cwe.mitre.org/data/definitions/526.html), [CWE-770](https://cwe.mitre.org/data/definitions/770.html)
- [QEMU `linux-user/syscall.c`](https://gitlab.com/qemu-project/qemu/-/blob/master/linux-user/syscall.c) — the `TARGET_NR_prlimit64` handler
- [CVE-2026-48592 — Oban Web missing authorization](https://basefortify.eu/cve_reports/2026/05/cve-2026-48592.html); [CWE-862](https://cwe.mitre.org/data/definitions/862.html) (2.3.1 addendum)
- [CVE-2017-12635 — CouchDB](https://docs.couchdb.org/en/stable/cve/2017-12635.html); [Bishop Fox — JSON interoperability vulnerabilities](https://bishopfox.com/blog/json-interoperability-vulnerabilities); [CWE-436](https://cwe.mitre.org/data/definitions/436.html) (2.3.2 addendum)
- [CVE-2014-3639](https://www.cve.org/CVERecord?id=CVE-2014-3639) and [CVE-2014-3638](https://www.cve.org/CVERecord?id=CVE-2014-3638) — D-Bus; [CVE-2018-16865](https://www.cve.org/CVERecord?id=CVE-2018-16865) — systemd-journald; [CWE-130](https://cwe.mitre.org/data/definitions/130.html), [CWE-636](https://cwe.mitre.org/data/definitions/636.html) (2.3.3 addendum)
- [CVE-2007-6750](https://www.cve.org/CVERecord?id=CVE-2007-6750) — Apache, "partial HTTP requests, as demonstrated by Slowloris" (2.3.4 addendum)
