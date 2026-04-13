# HTTP API Guide

Daimon exposes a REST API on port 8090 (configurable via `serve [port]`).

All responses are JSON. All POST bodies are JSON. Connection is closed after each response.

## Health

```
GET /v1/health
→ {"status":"ok","agents":0,"mcp_tools":0,"edge_nodes":0}
```

## Agents

```
# List agents
GET /v1/agents
→ {"agents":[{"id":1,"name":"my-agent","status":0,"pid":0}],"count":1}

# Register agent
POST /v1/agents
{"name":"my-agent"}
→ 201 {"id":1,"name":"my-agent","status":0}

# Get agent
GET /v1/agents/1
→ {"id":1,"name":"my-agent","status":0,"pid":0}
```

Agent status values: 0=Pending, 1=Starting, 2=Running, 3=Paused, 4=Stopping, 5=Stopped, 6=Failed.

## MCP Tools

```
# List tools
GET /v1/mcp/tools
→ {"tools":[{"name":"scan","description":"port scanner"}],"count":1}

# Register external tool
POST /v1/mcp/tools
{"name":"scan","description":"port scanner","callback_url":"http://localhost:9000"}
→ 201 {"ok":true}

# Call tool
POST /v1/mcp/call
{"name":"scan"}
→ {"content":[...],"isError":false}

# Deregister
DELETE /v1/mcp/tools/scan
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

# Get task
GET /v1/scheduler/tasks/2
→ {"task_id":"2","name":"train-model","priority":7,"status":"Queued"}

# Cancel task
POST /v1/scheduler/tasks/2/cancel
→ {"ok":true}

# Schedule pending tasks
POST /v1/scheduler/schedule
→ {"decisions":[{"task_id":"2","assigned_node":"worker-1","reason":"best-fit"}]}

# Scheduler stats
GET /v1/scheduler/stats
→ {"total_tasks":1,"queued":0,"running":0,"completed":1,"failed":0}
```

## Metrics

```
GET /v1/metrics
→ {"agents":1,"mcp_tools":2,"vector_entries":5,"edge_nodes":3,"federation_nodes":0}
```

## Error Responses

| Status | Meaning |
|---|---|
| 400 | Bad Request — missing/invalid field |
| 404 | Not Found — unknown route or ID |
| 413 | Payload Too Large — body > 64 KB |
| 422 | Unprocessable Entity — validation failure |
| 429 | Too Many Requests — rate limit (120/min per IP) |
| 501 | Not Implemented — chunked Transfer-Encoding |

All errors return `{"error":"message","code":NNN}`.

## Rate Limiting

120 requests per minute per source IP. Sliding window. Returns 429 when exceeded.

## Security

- All user-controlled strings in responses are JSON-escaped.
- Content-Length is validated; Transfer-Encoding is rejected.
- Maximum request size: 64 KB.
