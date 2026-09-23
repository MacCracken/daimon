# Architecture Overview

## What daimon is

Daimon is the AGNOS agent orchestrator — a single-binary service that manages the lifecycle of AI agents, schedules work across nodes, and provides HTTP, IPC, and MCP interfaces for the AGNOS ecosystem.

## Module Map

```
src/  (one compilation unit — main.cyr includes the per-domain modules below;
      cyrius flattens the includes into one global scope. Split out of the
      former 4.1k-line main.cyr in 1.2.8; module/include order matches the
      original source order.)
│
main.cyr   Preamble (syscall constants) + module includes + the `main` serve loop
│
├── error.cyr        Error codes (enum) + HTTP status mapping
├── config.cyr         Service configuration (listen_addr, port, data_dir, max_agents)
│
├── agent.cyr          Agent lifecycle (on the API since 2.3.0)
│   ├── AgentHandle        Snapshot: id, name, type, status, pid, exit_code, resources
│   ├── agent_start/pause/resume/reap   Process management; signals reach the agent's group
│   ├── agent_stop_begin/step   (2.3.4) a stop the event loop advances (grace, SIGKILL, reap)
│   ├── agent_find_executable   type → agnos-agent-<type>-agent (never from a request)
│   ├── read_vm_rss/cpu_time/fds/threads   /proc resource monitoring (AGNOS: proclist#99)
│   ├── agent_spawn_with_limits   fork/exec: its own process group, PDEATHSIG, closed descriptors,
│   │                             fd 3 the channel, optional output pipe, /dev/null stdin, SIGPIPE
│   │                             reset, RLIMIT_AS + RLIMIT_CPU + daimon's original NOFILE,
│   │                             exec failure reported synchronously
│   └── _agent_start_agnos   (2.4.0) spawn_path#43 with a channel endowed (CH_ENDOW); no limits
│                            (agnos has none: audited, limits_enforced false)
│
├── sched.cyr          Task start / complete over samay (2.3.1)
│   └── sched_task_start / sched_task_complete   SCHEDULED → RUNNING → COMPLETED/FAILED, capacity returned
│
├── supervisor.cyr     Health monitoring
│   ├── CircuitBreaker     Closed → Open → HalfOpen state machine
│   ├── OutputCapture      Ring buffer for stdout/stderr
│   ├── ResourceQuota      Memory/CPU warning + kill thresholds
│   └── AgentHealth        Per-agent health tracking
│
├── memory.cyr         Per-agent key-value store
│   ├── AgentMemoryStore   Filesystem-backed, atomic write (tmp+rename)
│   ├── validate_key       Path traversal prevention
│   └── sanitize_key       Filename-safe transformation
│
├── vector_store.cyr   Embedded vector search
│   ├── cosine_similarity  f64 dot product / magnitude
│   ├── normalize_vec      Unit length normalization
│   ├── VectorIndex        Brute-force cosine search with ranking
│   └── VectorEntry        id, embedding, content, metadata
│
├── rag.cyr            Retrieval-augmented generation
│   ├── chunk_text         Overlapping text chunking
│   ├── tokenize           Alphanumeric lowercase tokenizer
│   ├── rag_ingest_text    Chunk → hash-embed → index
│   └── rag_query_text     Embed → search → format context
│
├── mcp.cyr            MCP tool registry
│   ├── McpHostRegistry    Builtin + external tool maps
│   ├── mcp_tool_new       Tool descriptor (name, description, schema)
│   ├── validate_callback_url   SSRF protection
│   └── json_escape_str    Response injection prevention
├── mcp_builtin.cyr    Builtin in-process MCP dispatch (bote libro audit tools)
│   ├── mcp_libro_init     Bind bote's libro tools to the audit chain + register them
│   └── mcp_dispatch_builtin   Route builtin calls to bote handlers
├── audit.cyr          Audit trail over a hash-linked libro chain
│   ├── daimon_audit_init  Create the chain + genesis entry (backs libro_* tools)
│   └── daimon_audit / daimon_audit_agent   Append lifecycle + security events
│
├── screen.cyr         Capture management
│   ├── CapturePermissionManager   Per-agent permissions + rate limiting
│   └── RecordingManager   Session lifecycle (active/paused/stopped)
│
├── scheduler.cyr      Task scheduling
│   ├── ScheduledTask      State machine (Queued→Scheduled→Running→Completed/Failed/Cancelled/Preempted)
│   ├── NodeCapacity       Resource fitting, reserve/release
│   ├── TaskScheduler      Best-fit bin-packing, schedule_pending
│   └── scheduler_preempt_check   Priority preemption analysis
├── cron.cyr           CronScheduler — interval-based recurring triggers
│
├── federation.cyr     Multi-node clustering
│   ├── FederationNode     Node with Raft role + capabilities
│   ├── FederationManager  Heartbeat health, election, step-down
│   ├── fed_score_node     4-factor weighted placement (resource/locality/load/affinity)
│   └── fed_place_agent    Best-node selection
├── fed_vector_store.cyr   FederatedVectorStore — collection replicas, cross-node merge + dedup
│
├── edge.cyr           Edge fleet management
│   ├── EdgeNode           Status, capabilities, GPU inventory
│   ├── EdgeFleetManager   Register (with validation), heartbeat, health check, decommission
│   └── edge_fleet_stats   Aggregation across fleet
│
├── ipc.cyr            Inter-process communication
│   ├── IpcMessage         one freelist block, freed by the last queue holding it (2.3.4)
│   ├── MessageBus         id / first-wins name / broadcast routing; 100 per queue, 64 MiB total
│   ├── RpcRegistry        Method registration + lookup
│   ├── agent channels     (2.3.3) a socketpair per started agent, the agent's end on its fd 3;
│   │                      read by the event loop (epoll), parsed in a per-frame arena, answered
│   │                      with the routing outcome (ACK / NACK_QUEUE_FULL / NACK_NO_TARGET).
│   │                      AGNOS (2.4.0): a chan_op pair, the same stream in 64-byte records
│   └── captured output    (2.3.4, --agent-output capture) a 32 KiB ring per agent
│
├── app.cyr          Global service state + composition root
│   └── app_init           Initialize all subsystems
├── http.cyr         HTTP plumbing; request bodies (2.3.2): http_body_json + http_json_str/_int/_has/_text —
│                    the typed parser, strings decoded, one JSON object, no duplicate keys or U+0000
│   ├── http_parse_*       Method, path, query params, body, Content-Length
│   └── json_escape_str    Output encoding (VULN-002)
├── api.cyr          Service-level endpoints (health, metrics)
├── api_agent.cyr    Agent lifecycle endpoints
├── api_mcp.cyr      MCP tool registry + dispatch endpoints
├── api_rag.cyr      RAG ingest/query endpoints
├── api_edge.cyr     Edge fleet endpoints
├── api_sched.cyr    Scheduler endpoints, incl. task start/complete + a node's work list  (44 method + path routes in src/router.cyr)
├── router.cyr       http_route — HTTP method/path dispatch; agent and task control refuse Origin (403)
├── server.cyr       Server lifecycle
│   ├── rate_check         Per-IP 120 req/min sliding window
│   ├── handle_request     Host allowlist + cross-site-write refusal (VULN-012), then the router
│   ├── server_bind_addr   config listen_addr (127.0.0.1 unless serve --listen)
│   ├── server_loop        (2.3.4) daimon's own event loop: one thread, one epoll set over the
│   │                      listener, connections (non-blocking reads) and agent channels; a tick
│   │                      for deadlines, deferred answers (server_defer) and agents_tick.
│   │                      AGNOS (2.4.0): the same loop, polled, yielding with pause#14
│   └── server_detach      (2.3.4) a handler's work in a child process (MCP forwards, web_fetch);
│                          the loop relays its answer, 504 after 60 s
│
└── main.cyr        Entry point
    ├── serve(port)        dispatches to server.cyr
    └── CLI                serve [port] [--async] [--trace] [--agents-dir DIR] [--listen ADDR]
                           [--agent-env inherit|minimal] [--agent-output inherit|capture], version, help
```

## Data Flow

```
Client (HTTP)
  │
  ▼
epoll → accept → read (non-blocking) → complete? → smuggling checks → Rate / Host / Origin → Route
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
  └─ fd 3 (AGNOS_IPC_FD) ←socketpair→ daimon's event loop
       │   length-prefixed JSON; one reply byte per frame (ACK / NACK)
       ▼ as the loop reads it
     MessageBus → per-agent queues (id, name, "*" broadcast)
     audit chain ← channel faults (ipc.frame.*)
```

daimon is single-threaded. Its own event loop (2.3.4, [ADR-006](../adr/006-own-event-loop.md))
reads the channels along with HTTP, so a frame is answered and routed without waiting for a
request. 2.3.3 read them on a thread, which put the heap lock on every allocation.

On AGNOS (2.4.0, [ADR-007](../adr/007-daimon-on-agnos.md)) the same flow uses the kernel's
primitives: `spawn_path` for fork/exec, `proclist` for `/proc`, `kill` (a pending signal the agent
reads) for pidfd, and a `chan_op` pair, the agent's end endowed at spawn, for the socketpair. The
loop polls and yields with `pause`.

## Consumers

Every AGNOS agent (over the HTTP API and, once started by daimon, its channel on fd 3), hoosh, agnoshi, aethersafha, and any consumer app that talks to the HTTP API.

## Key Design Decisions

1. **Single compilation unit, multi-file source** — `src/main.cyr` `include`s 29 per-domain `src/*.cyr` modules (from the 1.2.8 monolith split, + `mcp_builtin.cyr` / `audit.cyr` at 1.3.0, `secmem.cyr` at 1.3.2, `trace.cyr` at 1.3.3, `sched.cyr` at 2.3.1; the largest are `agent.cyr` at ~800 lines and `ipc.cyr` at ~700). Cyrius flattens the includes into one global scope and compiles in one pass; no separate library crate. Contiguous module splits preserve original source order (byte-identical); the HTTP route handlers were regrouped by domain (pure functions, so order-independent), keeping behavior identical.
2. **daimon's own event loop, sandhi's HTTP** (2.3.4, ADR-006) — `server_loop` owns accept and reads (one thread, epoll). Requests are framed, smuggling-checked and answered with sandhi's public functions, as sandhi's own loops did through 2.3.3. `serve --async` is accepted and runs the same loop. On AGNOS (2.4.0, ADR-007) the loop polls instead of waiting on epoll, and yields with `pause`. Single trust domain.
3. **Bump allocator** — fast allocation, no individual free. Single trust domain (see VULN-007 security gate for multi-tenant).
4. **Everything is i64** — Cyrius type system. Structs are manually laid out with `alloc()` + `store64()`/`load64()` at fixed offsets.
5. **pidfd for signals** — race-free process management on Linux 5.3+, with `kill()` fallback.
6. **Dependencies** — a Cyrius stdlib subset plus a small set of external deps. `[deps].stdlib` (in dependency order): `string, fmt, math, alloc, vec, str, syscalls, io, fs, hashmap, tagged, bayan, net, mmap, dynlib, fdlopen, tls, sigil, sandhi, args, chrono, fnptr, process, async, thread, callback, assert, bench, freelist`. `sigil` is declared here because 6.3.43's `tls_native_lowlevel` references `sha384_init_into` (defined in sigil); `tls`/`mmap`/`dynlib`/`fdlopen` are present for compile-time symbol resolution only (pulled in transitively by sandhi's bundle). Vendored stdlib from the cyrius pin via `cyrius lib sync`: sandhi 1.7.0, sigil 3.10.0. External git dep via `cyrius deps`: sakshi 2.4.3 (structured logging/tracing). `lib/` is gitignored and repopulated by `cyrius lib sync` + `cyrius deps`; `cyrius.lock` locks 61 deps.
