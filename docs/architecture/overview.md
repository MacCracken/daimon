# Architecture Overview

## What daimon is

Daimon is the AGNOS agent orchestrator — a single-binary service that manages the lifecycle of AI agents, schedules work across nodes, and provides HTTP, IPC, and MCP interfaces for the AGNOS ecosystem.

## Module Map

```
src/main.cyr (4,141 LOC, single compilation unit)
│
├── error          Error codes (enum) + HTTP status mapping
├── config         Service configuration (listen_addr, port, data_dir, max_agents)
│
├── agent          Agent lifecycle
│   ├── AgentHandle        Snapshot: id, name, status, pid, resources
│   ├── agent_start/stop/pause/resume   Process management (fork/exec, pidfd signals)
│   ├── read_vm_rss/cpu_time/fds/threads   /proc resource monitoring
│   └── agent_spawn_with_limits   RLIMIT_AS + RLIMIT_CPU enforcement
│
├── supervisor     Health monitoring
│   ├── CircuitBreaker     Closed → Open → HalfOpen state machine
│   ├── OutputCapture      Ring buffer for stdout/stderr
│   ├── ResourceQuota      Memory/CPU warning + kill thresholds
│   └── AgentHealth        Per-agent health tracking
│
├── memory         Per-agent key-value store
│   ├── AgentMemoryStore   Filesystem-backed, atomic write (tmp+rename)
│   ├── validate_key       Path traversal prevention
│   └── sanitize_key       Filename-safe transformation
│
├── vector_store   Embedded vector search
│   ├── cosine_similarity  f64 dot product / magnitude
│   ├── normalize_vec      Unit length normalization
│   ├── VectorIndex        Brute-force cosine search with ranking
│   └── VectorEntry        id, embedding, content, metadata
│
├── rag            Retrieval-augmented generation
│   ├── chunk_text         Overlapping text chunking
│   ├── tokenize           Alphanumeric lowercase tokenizer
│   ├── rag_ingest_text    Chunk → hash-embed → index
│   └── rag_query_text     Embed → search → format context
│
├── mcp            MCP tool registry
│   ├── McpHostRegistry    Builtin + external tool maps
│   ├── mcp_tool_new       Tool descriptor (name, description, schema)
│   ├── validate_callback_url   SSRF protection
│   └── json_escape_str    Response injection prevention
│
├── screen         Capture management
│   ├── CapturePermissionManager   Per-agent permissions + rate limiting
│   └── RecordingManager   Session lifecycle (active/paused/stopped)
│
├── scheduler      Task scheduling
│   ├── ScheduledTask      State machine (Queued→Scheduled→Running→Completed/Failed/Cancelled/Preempted)
│   ├── NodeCapacity       Resource fitting, reserve/release
│   ├── TaskScheduler      Best-fit bin-packing, schedule_pending
│   ├── CronScheduler      Interval-based recurring triggers
│   └── scheduler_preempt_check   Priority preemption analysis
│
├── federation     Multi-node clustering
│   ├── FederationNode     Node with Raft role + capabilities
│   ├── FederationManager  Heartbeat health, election, step-down
│   ├── fed_score_node     4-factor weighted placement (resource/locality/load/affinity)
│   ├── fed_place_agent    Best-node selection
│   └── FederatedVectorStore   Collection replicas, cross-node merge + dedup
│
├── edge           Edge fleet management
│   ├── EdgeNode           Status, capabilities, GPU inventory
│   ├── EdgeFleetManager   Register (with validation), heartbeat, health check, decommission
│   └── edge_fleet_stats   Aggregation across fleet
│
├── ipc            Inter-process communication
│   ├── IpcMessage         Source, target, type, payload, timestamp
│   ├── MessageBus         Named routing, broadcast, direct send
│   ├── RpcRegistry        Method registration + lookup
│   ├── AgentIpc           Unix domain socket (bind, accept, send)
│   └── SO_PEERCRED        UID verification on accept
│
├── api            HTTP REST API (port 8090)
│   ├── 24 endpoints       health, agents, mcp, rag, edge, scheduler, metrics
│   ├── http_parse_*       Method, path, query params, body, Content-Length
│   ├── rate_check         Per-IP 120 req/min sliding window
│   └── json_escape_str    Output encoding
│
└── main           Entry point
    ├── app_init           Initialize all subsystems
    ├── serve(port)        TCP accept loop
    └── CLI                serve, version, help
```

## Data Flow

```
Client (HTTP)
  │
  ▼
TCP Accept → Rate Check → Parse Request → Route
  │                                         │
  ├─ /v1/agents ────────► AgentHandle map ──┤
  ├─ /v1/mcp/* ─────────► McpHostRegistry ──┤
  ├─ /v1/rag/* ─────────► RagPipeline ──────┤
  ├─ /v1/edge/* ────────► EdgeFleetManager ──┤
  ├─ /v1/scheduler/* ──► TaskScheduler ─────┤
  └─ /v1/metrics ──────► All subsystems ────┘
                                         │
                                    JSON Response
                                         │
                                         ▼
                                      Client
```

```
Agent Process
  │
  ├─ fork/exec with RLIMIT_AS + RLIMIT_CPU
  ├─ /proc/{pid}/status → VmRSS, threads, fds
  ├─ pidfd_open → race-free signal delivery
  └─ Unix socket ←→ AgentIpc (length-prefixed JSON, ACK/NACK)
       │
       └─► MessageBus → named routing / broadcast
           RpcRegistry → method dispatch
```

## Consumers

Every AGNOS agent, hoosh, agnoshi, aethersafha, and any consumer app that talks to the HTTP API or connects via Unix domain sockets.

## Key Design Decisions

1. **Single compilation unit** — all modules in `src/main.cyr`. Cyrius compiles everything in one pass; no separate library crate.
2. **Synchronous HTTP** — single-threaded TCP accept loop. No async runtime. Sufficient for orchestrator workloads; async deferred to future Cyrius stdlib maturity.
3. **Bump allocator** — fast allocation, no individual free. Single trust domain (see VULN-007 security gate for multi-tenant).
4. **Everything is i64** — Cyrius type system. Structs are manually laid out with `alloc()` + `store64()`/`load64()` at fixed offsets.
5. **pidfd for signals** — race-free process management on Linux 5.3+, with `kill()` fallback.
6. **No external dependencies** — 17 Cyrius stdlib modules, zero external crates.
