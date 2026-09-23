# HTTP API Guide

Daimon exposes a REST API on `127.0.0.1:8090`. Set the port with `serve [port]` and the bind
address with `serve --listen ADDR`. Through 2.2.3 daimon bound every interface; since 2.3.0 it binds
loopback unless told otherwise. `--listen 0.0.0.0` opens every interface. The API has no
authentication, so do that only behind a firewall.

All responses are JSON. All POST bodies are JSON. Connection is closed after each response.

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
curl -i -H "traceparent: 00-<32hex trace-id>-<16hex span-id>-01" http://localhost:8090/v1/health
→ ... X-Trace-Id: <32-hex trace-id>
```

Tracing is off by default (no `X-Trace-Id`, no span emission).

## Health

```
GET /v1/health
→ {"status":"ok","agents":0,"mcp_tools":0,"edge_nodes":0}
```

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
→ {"id":1,"name":"my-agent","type":"User","status":2,"pid":4242,"exit_code":null}
→ 409 not Pending/Stopped/Failed · 422 no executable for its type · 501 on AGNOS (until 2.4.x)

# Stop — SIGTERM, up to 5 s to exit, then SIGKILL; always 200
POST /v1/agents/1/stop
→ {"id":1,"name":"my-agent","type":"User","status":5,"pid":0,"exit_code":0}

# Pause / resume — SIGSTOP / SIGCONT
POST /v1/agents/1/pause      → status 3 · 409 unless Running
POST /v1/agents/1/resume     → status 2 · 409 unless Paused

# Delete — refused while the agent has a process
DELETE /v1/agents/1
→ {"ok":true} · 409 while it has a process
```

Agent status values: 0=Pending, 1=Starting, 2=Running, 3=Paused, 4=Stopping, 5=Stopped, 6=Failed.
A process that exits on its own is collected on the next agent request. Exit 0 leaves the agent
Stopped; a non-zero exit, or death by a signal, leaves it Failed. A Failed agent can be started
again. `exit_code` is null until a process has exited, and 128 + the signal number if a signal
killed it (137 = SIGKILL).

**What runs.** The request never names an executable. daimon runs `agnos-agent-<type>-agent`
(`system`, `user` or `service`) from `/usr/lib/agnos/agents`, then `/opt/agnos/agents`, else
`/usr/bin/agnos-agent-runner`. The argv is `--agent-id <id> --agent-name <name>`. The operator can
replace the search with `serve --agents-dir DIR`, where the runner is `DIR/agnos-agent-runner`. The
child:
- has stdin from `/dev/null`;
- inherits no descriptor above stderr;
- starts with SIGPIPE at its default;
- gets daimon's environment;
- runs under its supervisor quota as rlimits: 1 GiB address space and 3600 s CPU by default. A
  limit that cannot be applied refuses the start with a 500.

**Browsers may not control agents.** start / stop / pause / resume / DELETE answer **403** to any
request carrying an `Origin` header. A web page can send a cross-origin `text/plain` POST with no
CORS preflight, and could otherwise drive agents from a browser (2.3.0 audit, VULN-012). curl,
agents and other native clients send no `Origin`.

## MCP Tools

```
# List tools (inputSchema is raw JSON Schema; {} / {"type":"object"} when unset)
GET /v1/mcp/tools
→ {"tools":[{"name":"scan","description":"port scanner","inputSchema":{"type":"object","properties":{"target":{"type":"string"}},"required":["target"]}}],"count":1}

# Register external tool — inputSchema is optional (alias: input_schema),
# stored verbatim, defaults to {} when omitted (back-compatible).
POST /v1/mcp/tools
{"name":"scan","description":"port scanner","callback_url":"http://localhost:9000",
 "inputSchema":{"type":"object","properties":{"target":{"type":"string"}},"required":["target"]}}
→ 201 {"ok":true}

# Call tool
POST /v1/mcp/call
{"name":"scan"}
→ {"content":[...],"isError":false}

# Deregister
DELETE /v1/mcp/tools/scan
→ {"ok":true}

# Built-in tools — five libro audit-chain tools ship as builtins
# (libro_query / libro_verify / libro_export / libro_proof / libro_retention),
# listed by GET /v1/mcp/tools alongside any external tools. They dispatch
# in-process over daimon's audit chain and return the tool's JSON verbatim.
POST /v1/mcp/call
{"name":"libro_verify","arguments":{}}
→ {"ok":true}
```

## RAG Pipeline

```
# Ingest text
POST /v1/rag/ingest
{"text":"Rust is a systems programming language","metadata":"source1"}
→ 201 {"chunk_ids":[1,2]}

# Query
POST /v1/rag/query
{"query":"rust safety"}
→ {"formatted_context":"Use the following context..."}
```

## Edge Fleet

```
# Register node
POST /v1/edge/nodes
{"name":"edge-1"}
→ 201 {"id":"1"}

# List nodes (optional ?status=online|suspect|offline|updating|decommissioned)
GET /v1/edge/nodes
→ {"nodes":[...]}

# Get node
GET /v1/edge/nodes/1
→ {"id":"1","name":"edge-1","status":"Online","active_tasks":0}

# Heartbeat
POST /v1/edge/nodes/1/heartbeat
{"active_tasks":"3","tasks_completed":"10"}
→ {"ok":true}

# Decommission
POST /v1/edge/nodes/1/decommission
→ {"ok":true}

# Fleet stats
GET /v1/edge/stats
→ {"total":1,"online":1,"suspect":0,"offline":0,"active_tasks":0,"tasks_completed":0}
```

## Scheduler

```
# Register compute node
POST /v1/scheduler/nodes
{"node_id":"worker-1","total_cpu":"8","total_memory_mb":"16384"}
→ 201 {"ok":true}

# Submit task
POST /v1/scheduler/tasks
{"name":"train-model","agent_id":"agent-1","priority":"7"}
→ 201 {"task_id":"2"}

# List tasks
GET /v1/scheduler/tasks
→ {"stats":{"total_tasks":1,"queued":1,"running":0,...}}

# Get task — agent_id, node (where it was placed) and fail_reason since 2.3.1
GET /v1/scheduler/tasks/2
→ {"task_id":"2","name":"train-model","priority":7,"status":"Queued","agent_id":"agent-1","node":null,"fail_reason":null}

# Cancel task
POST /v1/scheduler/tasks/2/cancel
→ {"ok":true}

# An executor's work list: the node's Scheduled and Running tasks (2.3.1)
GET /v1/scheduler/nodes/worker-1/tasks
→ {"node_id":"worker-1","tasks":[{"task_id":"2",...,"status":"Scheduled","node":"worker-1",...}],"count":1}
→ 404 unknown node

# Start — the executor reports it running: Scheduled → Running (2.3.1)
POST /v1/scheduler/tasks/2/start
→ {"task_id":"2",...,"status":"Running",...}
→ 404 · 409 unless Scheduled · 403 from a browser

# Complete — and done: Running → Completed, or Failed with a reason (2.3.1)
POST /v1/scheduler/tasks/2/complete                                  (no body = completed)
POST /v1/scheduler/tasks/2/complete {"status":"failed","reason":"disk full"}
→ {"task_id":"2",...,"status":"Failed",...,"fail_reason":"disk full"}
→ 400 status not completed/failed · 404 · 409 unless Running · 403 from a browser

# Schedule pending tasks
POST /v1/scheduler/schedule
→ {"decisions":[{"task_id":"2","assigned_node":"worker-1","reason":"best-fit"}]}

# Scheduler stats — the averages are 0 until a task has started (2.3.1)
GET /v1/scheduler/stats
→ {"total_tasks":1,"queued":0,"running":0,"completed":1,"failed":0,"average_wait_time_ms":54,"average_run_time_ms":82}
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
→ {"agents":1,"mcp_tools":2,"vector_entries":5,"edge_nodes":3,"federation_nodes":0}
```

## Error Responses

| Status | Meaning |
|---|---|
| 400 | Bad Request — missing/invalid field |
| 403 | Forbidden — agent or task control from a browser (a request carrying `Origin`) |
| 404 | Not Found — unknown route or ID |
| 405 | Method Not Allowed — a route exists, not for this method |
| 409 | Conflict — the agent's status does not allow the action, a name is taken, or the agent limit is reached |
| 413 | Payload Too Large — body > 64 KB |
| 422 | Unprocessable Entity — validation failure |
| 429 | Too Many Requests — rate limit (120/min per IP) |
| 500 | Internal Server Error — e.g. an agent's rlimits could not be applied, or its executable could not be run |
| 501 | Not Implemented — chunked Transfer-Encoding; agent processes on AGNOS (until 2.4.x) |

All errors return `{"error":"message","code":NNN}`.

## Rate Limiting

120 requests per minute per source IP. Sliding window. Returns 429 when exceeded.

## Security

- There is **no authentication** (roadmap 2.5.x). The defaults limit who can reach the API. It binds
  127.0.0.1, and agent control refuses browser-originated requests. Other mutating routes still
  accept a cross-origin `text/plain` POST.
- All user-controlled strings in responses are JSON-escaped.
- Content-Length is validated; Transfer-Encoding is rejected.
- Maximum request size: 64 KB.
- An agent's executable is chosen by daimon from its type, never by a request. See Agents.
- **Known issue (until 2.3.2):** string values in request bodies are stored with their JSON escapes
  undecoded. A name sent as `"say \"hi\""` is kept, and echoed back, as `say \"hi\"`. Plain ASCII
  without quotes, backslashes or `\u` escapes is unaffected.
