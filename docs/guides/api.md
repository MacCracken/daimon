# HTTP API Guide

Daimon exposes a REST API on `127.0.0.1:8090`. Set the port with `serve [port]` and the bind
address with `serve --listen ADDR`. Through 2.2.3 daimon bound every interface; since 2.3.0 it binds
loopback unless told otherwise. `--listen 0.0.0.0` opens every interface. The API has no
authentication, so do that only behind a firewall.

Answers are JSON, except the few refusals that have no body (see Error Responses). POST bodies are
JSON. The connection is closed after each answer.

**Who may call it** (2.3.4, VULN-012):
- While daimon listens on loopback (the default), a request must name a loopback host (`Host:
  127.0.0.1`, `localhost` or `[::1]`, any port); any other name gets **403**. This is what defeats
  DNS rebinding, where a web page's requests reach 127.0.0.1 under the attacker's hostname. With
  `--listen` beyond loopback the check is off: the operator chose the names daimon answers to.
- A request that changes state (POST, PUT, DELETE) and carries an `Origin` from another site gets
  **403** on every route. Such a request is what a web page sends with no CORS preflight (a
  `text/plain` POST). A page served from a loopback host (`http://127.0.0.1:…`, `http://localhost:…`)
  may still write. Agent and task control refuse any `Origin`, as since 2.3.0. Clients that are not
  browsers send no `Origin`.

**One client does not hold the server** (2.3.4): daimon runs its own event loop and reads requests
without blocking, so a slow client holds only its own connection:
- a connection that sends nothing for 5 s, or has no whole request after 30 s, is closed;
- at most 128 connections are open at once (5 on AGNOS), and more wait in the kernel's queue;
- when all 128 are taken and another is waiting, the oldest request still arriving gives way, once
  it is 1 s old (2.4.2). It is closed unanswered and counted (`http_evicted`). One client trickling
  in 128 requests held every slot for 30 s before.

## Request bodies

Since 2.3.2 a body is **one JSON object**, read with a full JSON parser. It gets **400** when it:
- is not JSON (for example form-encoded), has trailing content, or is not an object;
- repeats a top-level key. Parsers disagree on which one wins, so daimon accepts neither;
- has U+0000 in a top-level string.

Values:
- **Strings are decoded**: `"say \"hi\" \\o/ \u00e9"` is stored, and echoed back, as
  `say "hi" \o/ é`. Before 2.3.2 the escapes were kept as typed.
- **A string field** takes a JSON string, or a number read as its text (`"agent_id": 3` is `"3"`).
- **An integer field** takes a number, a numeric string (`"priority": "7"`), or true / false as 1 / 0.
- **Only top-level fields are read.** A key inside a nested object never stands in for a top-level
  one. Nested values that daimon forwards, such as MCP `arguments`, are passed on untouched.

## Distributed tracing

When started with `serve --trace`, daimon participates in a distributed trace on
every request:

- **Inbound** — it adopts the full 128-bit trace-id from a W3C `traceparent`
  header (or an `X-Trace-Id` header: 32-hex = 128-bit, 16-hex = 64-bit),
  generating one if neither is present.
- **Response** — it echoes `X-Trace-Id: <32 hex>` (the full trace-id) and emits
  timed sakshi spans (`http.request`, `mcp.builtin` / `mcp.forward`) correlated
  under that id.
- **Outbound** — on an external MCP forward it propagates the trace context
  downstream as a fresh `traceparent` request header, so that endpoint joins the
  trace.

```
curl -i -H "traceparent: 00-<32hex trace-id>-<16hex span-id>-01" http://127.0.0.1:8090/v1/health
→ ... X-Trace-Id: <32-hex trace-id>
```

Tracing is off by default (no `X-Trace-Id`, no span emission).

## Health

```
GET /v1/health
→ {"status":"ok","agents":0,"mcp_tools":13,"mcp_resources":0,"mcp_prompts":0,"edge_nodes":0}
```

`mcp_tools` counts the 13 builtins as well as registered tools.

## Agents

```
# List agents
GET /v1/agents
→ {"agents":[{"id":1,"name":"my-agent","type":"User","status":0,"pid":0,"exit_code":null}],"count":1}

# Register agent — type is optional: System, User (default) or Service
POST /v1/agents
{"name":"my-agent","type":"User"}
→ 201 {"id":1,"name":"my-agent","type":"User","status":0}
→ 400 unknown type · 409 at max_agents (1000)

# Get agent
GET /v1/agents/1
→ {"id":1,"name":"my-agent","type":"User","status":0,"pid":0,"exit_code":null}

# Start — runs the agent's process (2.3.0)
POST /v1/agents/1/start
→ {"id":1,"name":"my-agent","type":"User","status":2,"pid":4242,"exit_code":null,"limits_enforced":true,"contained":true}
→ 409 not Pending/Stopped/Failed · 422 no executable for its type (on AGNOS also: a name with a space)

# Stop — SIGTERM, up to 5 s to exit, then SIGKILL; always 200, once the agent is gone
POST /v1/agents/1/stop
→ {"id":1,"name":"my-agent","type":"User","status":5,"pid":0,"exit_code":0}

# Pause / resume — SIGSTOP / SIGCONT
POST /v1/agents/1/pause      → status 3 · 409 unless Running
POST /v1/agents/1/resume     → status 2 · 409 unless Paused

# Delete — refused while the agent has a process
DELETE /v1/agents/1
→ {"ok":true} · 409 while it has a process

# Messages (2.3.4) — queue one for the agent; take (and remove) what is queued
POST /v1/agents/1/messages            {"type":"command","payload":{...}}   → 201 {"id":13}
POST /v1/agents/1/messages/take       {"max":10}                           → {"messages":[...],"count":1,"remaining":0}

# Captured output (2.3.4; serve --agent-output capture) — stdout and stderr since its latest start
GET /v1/agents/1/output
→ {"output":"hello\n","bytes":6,"kept":6} · 409 when output is not captured
```

Agent status values: 0=Pending, 1=Starting, 2=Running, 3=Paused, 4=Stopping, 5=Stopped, 6=Failed.
A process that exits on its own is collected within 100 ms (daimon's upkeep tick, 2.3.4). Exit 0 leaves the agent
Stopped; a non-zero exit, or death by a signal, leaves it Failed. A stop through the API leaves it
Stopped, whatever its exit code (143 when the SIGTERM killed it). A Failed agent can be started
again. `exit_code` is null until a process has exited, and 128 + the signal number if a signal
killed it (137 = SIGKILL).

**What runs.** The request never names an executable. daimon runs `agnos-agent-<type>-agent`
(`system`, `user` or `service`) from `/usr/lib/agnos/agents`, then `/opt/agnos/agents`, else
`/usr/bin/agnos-agent-runner`. The argv is `--agent-id <id> --agent-name <name>`. The operator can
replace the search with `serve --agents-dir DIR`, where the runner is `DIR/agnos-agent-runner`. The
child:
- has stdin from `/dev/null`;
- inherits no descriptor above stderr except **fd 3, its channel to daimon** (2.3.3);
- starts with SIGPIPE at its default;
- gets daimon's environment, plus `AGNOS_IPC_FD=3`;
- runs under its supervisor quota as rlimits: 1 GiB address space and 3600 s CPU by default. A
  limit that cannot be applied refuses the start with a 500.

**A stop does not hold the server** (2.3.4, VULN-014). Its answer comes when the agent is gone, up
to about 6 s for an agent that ignores SIGTERM. Other requests, and agents' channels, are served
meanwhile. A GET in between shows the agent at status 4 (Stopping). A stop, pause or resume reaches
the processes the agent started too: each agent leads its own process group. An agent is killed if
daimon itself dies (`PR_SET_PDEATHSIG`); daimon's registry is in memory, so it could never be
reached again.

**Containment** (2.4.2, [ADR-008](../adr/008-agent-containment-cgroup.md)). A process that calls
`setsid` leaves its agent's process group, and through 2.4.1 it outlived the agent's stop. Where
daimon's own cgroup is its to divide, each agent runs in a cgroup of its own, and its JSON says
`"contained":true` while it has a process. That means a systemd service with `Delegate=yes`, anything
the user's systemd runs (`systemd-run --user`), or a cgroup handed to daimon's user. Then:
- a stop, pause or resume reaches every process in the cgroup;
- when the agent's own process is collected, anything left in it gets SIGTERM, 5 s, then SIGKILL.
  **An agent's processes end with it**. Its cgroup is then removed, with any the agent made inside
  it;
- a daimon that was killed outright left its agents' leftovers running, and the next daimon started
  in the same cgroup ends them.

This holds processes that detach (`setsid`, a double fork, a daemon). It does not hold an agent that
acts against daimon: an agent runs as daimon's user, and may move its own processes into another
cgroup that user may write.

Elsewhere, agents run as before, and `"contained":false`. That covers daimon started from a login
shell (the session's cgroup is root's), and a cgroup whose `cgroup.procs` daimon may not write.
daimon warns once at startup, and audits `agent.cgroup.unavailable`.

**Environment and output** (2.3.4):
- `serve --agent-env minimal` gives agents only PATH, HOME, USER, LOGNAME, SHELL, TERM, TMPDIR, TZ,
  LANG, LANGUAGE, LC_*, XDG_RUNTIME_DIR and AGNOS_*, instead of daimon's whole environment (the
  default, `inherit`; VULN-015).
- `serve --agent-output capture` collects each agent's stdout and stderr: the last 32 KiB, served
  at `GET /v1/agents/{id}/output`. Invalid UTF-8 is shown as U+FFFD. The default, `inherit`, leaves
  them daimon's own.

**On AGNOS** (2.4.0, [ADR-007](../adr/007-daimon-on-agnos.md)) the same routes use the agnos
kernel's primitives. Some things differ until agnos closes the gaps filed with it
(`docs/development/issues/2026-09-23-*.md` in the agnos repo):
- **Room**: the machine has 16 process slots and 16 channels. Measured in daimon's guest test,
  12 agents run at once. A start with no free channel answers **503** ("no room for another
  agent"), and the agent stays as it was (2.4.1). A start that `spawn_path` refuses answers **500**
  ("its process table is full, or it cannot load the executable"): agnos does not say which.
- **Start**: `spawn_path` runs `<exe> --agent-id <id> --agent-name <name>` as one line split on
  spaces, at most 127 bytes. So a name with a space, or a longer line, answers **422**. The
  environment is daimon's (filtered by `--agent-env`), at most 16 entries and 1024 bytes, plus
  `AGNOS_IPC_FD` naming the channel's fd, which the kernel chooses.
- **No limits**: agnos has none, so the agent runs without its quota. Its JSON says
  `"limits_enforced":false`, and each start is audited `agent.limits.unenforced`.
- **Stop asks**: SIGTERM, which the agent sees on a signalfd. An agent that ignores it stays at status
  4 (Stopping), because no signal ends a process on agnos yet. Pause and resume answer **501**.
- **No captured output**: `serve --agent-output capture` exits 1, because an agnos child inherits
  daimon's whole fd table.
- **The listener** is on the NIC's address: agnos cannot bind 127.0.0.1. daimon warns and audits
  `http.listen.not_loopback`. Local clients reach it at the box's own address, not 127.0.0.1.
- **5 connections at once**, not 128. The kernel has 8 TCP slots for the whole machine: the
  listener takes one, and daimon's own calls to other servers need theirs.
- **The rate limit is shared.** agnos gives no peer address, so every client draws on one bucket of
  960 requests a minute (120 for each of 8 connections).
- **Answers are written 512 bytes at a time, 1 ms apart** (2.4.1; 512 bytes and the 1 ms since
  2.4.3, where it had been 1 KB and one yield). A TCP receive ring on agnos is 2 KB, and the
  kernel holds the CPU while a send waits for room, so a write a local client has no room for stops
  the machine. A client that reads keeps up. One that stops reading mid-answer can still stop it, and
  so, rarely, can one on a very slow machine. An answer goes at 512 KB/s at most.

**Browsers may not control agents.** start / stop / pause / resume / DELETE answer **403** to any
request carrying an `Origin` header. A web page can send a cross-origin `text/plain` POST with no
CORS preflight, and could otherwise drive agents from a browser (2.3.0 audit, VULN-012). curl,
agents and other native clients send no `Origin`.

**Agents talk back on fd 3** (2.3.3). A started agent writes length-prefixed JSON frames to its
channel and gets a one-byte reply for each. daimon puts accepted messages on its message bus, where
each registered agent has a queue. The wire format, the limits and what closes a channel are in
[agent-ipc.md](agent-ipc.md). The channel's traffic shows in `/v1/metrics`.

## MCP

daimon is an MCP host. It lists and calls its own 13 builtin tools, and the tools, resources and
prompts that external MCP servers register with it. A call to an external one is forwarded to the
server's `callback_url` as JSON-RPC 2.0 (`tools/call`, `resources/read`, `prompts/get`), with the
request body as its params.

### Built-in tools

| tools | what they do |
|---|---|
| `libro_query`, `libro_verify`, `libro_export`, `libro_proof`, `libro_retention` | daimon's audit chain (libro's tools, through bote): search it, verify its hash links, export it, prove an entry's inclusion, apply a retention policy |
| `web_fetch`, `web_search` | bote's web tools: a page's readable text (`{url}`, http or https only), and results from the SearXNG instance at `BOTE_SEARXNG_URL` (`{query, count?}`). Unset, `web_search` says so |
| `nein_status`, `nein_list`, `nein_validate`, `nein_diff` | nein's read-only firewall tools |
| `nein_allow`, `nein_deny` | nein's tools that change the live firewall. **Refused** until callers are authenticated (roadmap 2.5.x): `"access denied: tool gated by host policy"`, audited `nein.gate.deny` |

Every builtin answers with an MCP tool result. libro's tools return bare JSON, which daimon wraps
as the text of one content block, with `isError` true when that JSON says `"ok":false`. bote's and
nein's tools return their own content blocks, which pass through as they are. Every nein tool but
`nein_validate` runs `nft`, so it needs root and nftables. Without them, or on AGNOS, which has no
nftables, it answers `"isError":true`. No external registration may take a builtin's name (409).

### Tools

```
# List tools, the builtins included (inputSchema is raw JSON Schema; {} / {"type":"object"} when unset).
# GET /v1/mcp/manifest answers the same.
GET /v1/mcp/tools
→ {"tools":[{"name":"libro_verify","description":"Verify the libro chain's hash-link integrity","inputSchema":{"type":"object"}},...],"count":13}

# Register an external tool. inputSchema is optional (alias: input_schema), stored verbatim, {} when
# omitted. callback_url must be http:// or https:// (a scheme check, not a host allowlist).
POST /v1/mcp/tools
{"name":"scan","description":"port scanner","callback_url":"http://localhost:9000",
 "inputSchema":{"type":"object","properties":{"target":{"type":"string"}},"required":["target"]}}
→ 201 {"ok":true} · 400 no name or callback_url, or not http(s) · 409 a builtin's name
# Registering a name again replaces it, whoever registered it first (roadmap 2.5.x).

# Call a tool: MCP tools/call params
POST /v1/mcp/call
{"name":"libro_verify","arguments":{}}
→ {"content":[{"type":"text","text":"{\"ok\":true}"}],"isError":false}
{"name":"scan","arguments":{"target":"10.0.0.1"}}
→ the external server's result, verbatim
→ 400 unknown tool · 502 the server could not be reached, or answered wrongly · 504 no answer in 60 s

# Deregister
DELETE /v1/mcp/tools/scan
→ {"ok":true}
```

A call that waits on another server runs in a child process, so daimon keeps serving meanwhile.
That covers a forwarded call, `resources/read`, `prompts/get`, `web_fetch` and `web_search`. One
that has not finished within 60 s is answered 504 (2.3.4; on AGNOS since 2.4.1). On AGNOS a server
that sends nothing for 30 s is taken to have closed (502). Through 2.4.1 that was about a second
whenever the guest was busy (2.4.2). A JSON-RPC error from the server answers 200 with
`"isError":true` and its `code`.

### Resources and prompts (2.1.0)

A resource URI contains `://` and more slashes, so it travels in the body, never in the path.

```
# Register an external resource (mimeType, or mime_type; text/plain when omitted)
POST /v1/mcp/resources
{"uri":"file:///etc/motd","name":"motd","description":"message of the day","mimeType":"text/plain","callback_url":"http://localhost:9000"}
→ {"registered":"file:///etc/motd","count":1}

GET /v1/mcp/resources
→ {"resources":[{"uri":"file:///etc/motd","name":"motd","description":"message of the day","mimeType":"text/plain"}],"count":1}

# Read it: forwards resources/read to the server that registered it
POST /v1/mcp/resources/read     {"uri":"file:///etc/motd"}
→ the server's result, verbatim · 400 unknown uri · 502 · 504

POST /v1/mcp/resources/deregister     {"uri":"file:///etc/motd"}
→ {"deregistered":true}

# Prompts: the same shape, keyed by name. arguments is the prompt's argument list.
POST /v1/mcp/prompts
{"name":"greet","description":"say hello","arguments":[{"name":"who","required":true}],"callback_url":"http://localhost:9000"}
→ {"registered":"greet","count":1}

GET /v1/mcp/prompts
→ {"prompts":[{"name":"greet","description":"say hello","arguments":[{"name":"who","required":true}]}],"count":1}

POST /v1/mcp/prompts/get     {"name":"greet","arguments":{"who":"agnos"}}
→ the server's result, verbatim · 400 unknown prompt · 502 · 504

POST /v1/mcp/prompts/deregister     {"name":"greet"}
→ {"deregistered":true}
```

A forwarded read or get that the server answers with a JSON-RPC error is answered 502, with the
server's message and code.

## RAG Pipeline

```
# Ingest text: chunked, embedded and indexed (a longer text gives more chunk ids)
POST /v1/rag/ingest
{"text":"Rust is a systems programming language","metadata":"source1"}
→ 201 {"chunk_ids":[1]}

# Query: the best-matching chunks, formatted as context for a model
POST /v1/rag/query
{"query":"rust safety"}
→ {"formatted_context":"Use the following context to answer the question.\n\n---\n[1] Rust is a systems programming language\n\n---\n\nQuestion: rust safety"}
```

The index is in memory, so it starts empty each time daimon starts. `vector_entries` in
`/v1/metrics` counts it.

## Edge Fleet

```
# Register node — capabilities optional (2.3.4); defaults x86_64 / 4 / 4096 / 32768 / false
POST /v1/edge/nodes
{"name":"edge-1","arch":"aarch64","cpu_cores":4,"memory_mb":8192,"disk_mb":65536,"has_gpu":true}
→ 201 {"id":"1"} · 400 name taken · 422 an invalid capability

# List nodes (optional ?status=online|suspect|offline|updating|decommissioned; any other value lists all).
# Here status is a number: 0 Online, 1 Suspect, 2 Offline, 3 Updating, 4 Decommissioned.
GET /v1/edge/nodes
→ {"nodes":[{"id":"1","name":"edge-1","status":0}]}

# Get node
GET /v1/edge/nodes/1
→ {"id":"1","name":"edge-1","status":"Online","active_tasks":0,"arch":"aarch64","cpu_cores":4,
   "memory_mb":8192,"disk_mb":65536,"has_gpu":true}

# Heartbeat
POST /v1/edge/nodes/1/heartbeat
{"active_tasks":"3","tasks_completed":"10"}
→ {"ok":true}

# Decommission
POST /v1/edge/nodes/1/decommission
→ {"ok":true}

# Fleet stats
GET /v1/edge/stats
→ {"total":1,"online":1,"suspect":0,"offline":0,"updating":0,"decommissioned":0,"active_tasks":0,
   "tasks_completed":0,"cpu_cores":4,"memory_mb":8192,"gpu_nodes":1}
```

A node's health is worked out whenever the fleet is read. A node with no heartbeat for 30 s is
Suspect, and after 90 s it is Offline.

Edge node ids have their own sequence since 2.3.4. Before, they came from the agent counter, so
after five agents the first node was `"6"`. `cpu_cores`, `memory_mb` and `gpu_nodes` in the stats
total the nodes that can take work (not offline, not decommissioned). `arch` is 1–32 characters of
`a-z`, `0-9` and `_`. `cpu_cores` is 1–65536, and `memory_mb` and `disk_mb` are whole numbers.

## Scheduler

```
# Register compute node
POST /v1/scheduler/nodes
{"node_id":"worker-1","total_cpu":"8","total_memory_mb":"16384"}
→ 201 {"ok":true}

# Submit task. The id is samay's, a UUID.
POST /v1/scheduler/tasks
{"name":"train-model","agent_id":"agent-1","priority":"7"}
→ 201 {"task_id":"21cecebe-6079-4fe7-9618-e533df976a7d"}

# Task counts. This route lists no tasks: find a node's with its work list, below.
GET /v1/scheduler/tasks
→ {"stats":{"total_tasks":1,"queued":1,"running":0,"completed":0,"failed":0}}

# Get task — agent_id, node (where it was placed) and fail_reason since 2.3.1
GET /v1/scheduler/tasks/21cecebe-6079-4fe7-9618-e533df976a7d
→ {"task_id":"21cecebe-6079-4fe7-9618-e533df976a7d","name":"train-model","priority":7,"status":"Queued","agent_id":"agent-1","node":null,"fail_reason":null}

# Cancel task
POST /v1/scheduler/tasks/{id}/cancel
→ {"ok":true} · 404 unknown, or its status does not allow a cancel

# Schedule pending tasks: best fit over the registered nodes
POST /v1/scheduler/schedule
→ {"decisions":[{"task_id":"21cecebe-…","assigned_node":"worker-1","reason":"best-fit selection","score":1.0}]}

# An executor's work list: the node's Scheduled and Running tasks (2.3.1)
GET /v1/scheduler/nodes/worker-1/tasks
→ {"node_id":"worker-1","tasks":[{"task_id":"21cecebe-…","name":"train-model","priority":7,"status":"Scheduled","agent_id":"agent-1","node":"worker-1","fail_reason":null}],"count":1}
→ 404 unknown node

# Start — the executor reports it running: Scheduled → Running (2.3.1)
POST /v1/scheduler/tasks/{id}/start
→ {"task_id":"21cecebe-…",...,"status":"Running","node":"worker-1",...}
→ 404 · 409 unless Scheduled · 403 from a browser

# Complete — and done: Running → Completed, or Failed with a reason (2.3.1)
POST /v1/scheduler/tasks/{id}/complete                                  (no body = completed)
POST /v1/scheduler/tasks/{id}/complete {"status":"failed","reason":"disk full"}
→ {"task_id":"21cecebe-…",...,"status":"Failed",...,"fail_reason":"disk full"}
→ 400 status not completed/failed · 404 · 409 unless Running · 403 from a browser

# Scheduler stats — the averages are 0 until a task has started (2.3.1)
GET /v1/scheduler/stats
→ {"total_tasks":1,"queued":0,"running":0,"completed":0,"failed":1,"average_wait_time_ms":15,"average_run_time_ms":0}
```

A task's life through the API: submit → schedule places it on a node (Scheduled, holding that node's
capacity) → the executor on that node finds it with `GET /v1/scheduler/nodes/{id}/tasks`, reports it
started (Running), then reports it complete (Completed or Failed). Completing it returns the
capacity at once, so the next schedule can use it. `status` for complete accepts `completed` /
`failed` or `Completed` / `Failed`.

Anyone who can reach the API can report any task. Only browsers are refused (the Origin rule, as for
agents), and executor identity waits for authentication (roadmap 2.5.x).

## Metrics

```
GET /v1/metrics
→ {"agents":1,"mcp_tools":13,"mcp_resources":0,"mcp_prompts":0,"vector_entries":5,"edge_nodes":3,
   "federation_nodes":0,"ipc_messages":42,"ipc_refused":1,"bus_dropped":0,"ipc_channels":1,
   "bus_bytes":2048,"http_evicted":0}
```

`federation_nodes` is always 0 for now: no route adds a federation node.

Since 2.3.3:
- `ipc_messages` counts agent messages put on the bus;
- `ipc_refused` counts frames answered NACK, plus channels closed on a fault;
- `bus_dropped` counts messages not queued because a queue (100 per agent) or the bus (64 MiB) was
  full.

Since 2.3.4:
- `ipc_channels` is the number of open agent channels;
- `bus_bytes` is the bytes of messages the bus holds (freed as they are taken).

Since 2.4.2, `http_evicted` counts connections closed to make room while every slot was taken.

See [agent-ipc.md](agent-ipc.md).

## Error Responses

| Status | Meaning |
|---|---|
| 400 | Bad Request — missing/invalid field, or a body that is not one JSON object (see Request bodies) |
| 403 | Forbidden — a host name other than loopback's while daimon listens on loopback; a state change from another site; agent or task control from any browser |
| 404 | Not Found — unknown route or ID |
| 405 | Method Not Allowed — a route exists, not for this method |
| 409 | Conflict — the agent's status does not allow the action, a name is taken, the agent limit is reached, or output is not captured |
| 413 | Payload Too Large — body > 64 KB |
| 422 | Unprocessable Entity — validation failure |
| 429 | Too Many Requests — rate limit (120/min per IP), or an agent's message queue is full |
| 500 | Internal Server Error — e.g. an agent's rlimits could not be applied, or its executable could not be run |
| 501 | Not Implemented — chunked Transfer-Encoding; pause and resume on AGNOS |
| 502 | Bad Gateway — an MCP endpoint could not be reached or answered wrongly |
| 503 | Service Unavailable — on AGNOS: no free channel for another agent, or 4 calls to other servers still running |
| 504 | Gateway Timeout — an MCP call, `web_fetch` or `web_search` did not finish within 60 s |

A handler's error answers `{"error":"message","code":NNN}`. Four answers are shorter:
- an unknown route: `{"error":"not found"}`;
- a wrong method: `{"error":"method not allowed"}`;
- the rate limit: `{"error":"too many requests"}`;
- a request refused before it is read (400 for an ambiguous length, 413, 501 for chunked): no body.

## Rate Limiting

120 requests a minute from each source IP, counted in a fixed 60 s window that starts with its first
request. Past that, 429. A client whose address daimon cannot learn draws on one shared bucket of 960
a minute. That is every client on AGNOS, which gives no peer address.

## Security

- There is **no authentication** (roadmap 2.5.x). The defaults limit who can reach the API. It binds
  127.0.0.1, answers only to loopback host names while it does, and refuses state changes from
  other sites on every route. Agent and task control refuse any browser-originated request.
- All user-controlled strings in responses are JSON-escaped.
- Content-Length is validated; Transfer-Encoding is rejected.
- Maximum request size: 64 KB.
- An agent's executable is chosen by daimon from its type, never by a request. See Agents.
- Request bodies are parsed strictly (see Request bodies). Every string field is decoded, and a
  body with a duplicate top-level key or U+0000 in a top-level string is refused.
