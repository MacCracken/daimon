# Architecture Overview

## What daimon is

Daimon is the AGNOS agent orchestrator. It is a single binary that starts, supervises and stops AI
agents, hears them on a channel of their own, and serves them and their clients one HTTP API. Through
that API it also hosts MCP tools, schedules tasks across compute nodes, keeps a RAG index and tracks
an edge fleet.

## Module Map

```
src/  (one compilation unit: src/main.cyr includes the 29 modules below, in this order, and cyrius
      flattens them into one global scope. Split out of the former 4.1k-line main.cyr at 1.2.8.)
│
main.cyr             CLI: serve [port] [--async] [--trace] [--agents-dir DIR] [--listen ADDR]
│                    [--agent-env inherit|minimal] [--agent-output inherit|capture], version, help.
│                    `serve` → serve() in server.cyr; --async is accepted and runs the same loop
│
├── syscalls.cyr     The portability layer (2.1.6): daimon's syscall numbers per arch, and one call
│                    shape with per-target bodies — daimon_signal (pidfd, then kill), daimon_reap_code,
│                    daimon_unlink, daimon_yield_ms (pause#14 on agnos), daimon_now_ms (2.4.1: survives
│                    a refused TSC calibration), daimon_write_all (2.4.1; 512-byte pieces 1 ms apart
│                    on agnos since 2.4.3), daimon_agnos_recv_bound (2.4.2)
├── error.cyr        DAIMON_ERR_* codes + HTTP status mapping; error_json; json_escape_str /
│                    json_escape_text (output escaping, VULN-002); json_object_ok (the rule a request
│                    body and a channel frame share)
├── config.cyr       Service configuration (listen_addr, port, data_dir, max_agents); DAIMON_VERSION
├── audit.cyr        daimon's audit trail on a hash-linked libro chain (1.3.0)
│   └── daimon_audit / daimon_audit_agent   lifecycle + security events; the libro_* tools read the chain
├── secmem.cyr       secure_zero: scrubs buffers that held sensitive bytes (VULN-007's hygiene half)
├── trace.cyr        Distributed tracing over sakshi (serve --trace): a 128-bit trace-id adopted from
│                    traceparent / X-Trace-Id, echoed, and propagated on MCP forwards
│
├── agent.cyr        Agent lifecycle (on the API since 2.3.0)
│   ├── AgentHandle        Snapshot: id, name, type, status, pid, exit_code, resources, limits_enforced, contained
│   ├── agent_start/pause/resume/reap   Process management; signals reach the agent's group
│   ├── agent_stop_begin/step   (2.3.4) a stop the event loop advances (grace, SIGKILL, reap)
│   ├── agent_find_executable   type → agnos-agent-<type>-agent (never from a request)
│   ├── read_vm_rss / read_cpu_time_ms / count_fds / count_threads   /proc readings (AGNOS: proclist#99)
│   ├── agent_spawn_with_limits   fork/exec: its own process group, PDEATHSIG, closed descriptors,
│   │                             fd 3 the channel, optional output pipe, /dev/null stdin, SIGPIPE
│   │                             reset, RLIMIT_AS + RLIMIT_CPU + daimon's original NOFILE,
│   │                             exec failure reported synchronously
│   ├── containment    (2.4.2, ADR-008) a cgroup per agent where daimon's cgroup is its own:
│   │                  _agent_spawn moves the child in; _agent_cg_confirm checks it is there
│   │                  (/proc/<pid>/cgroup); _agent_signal_all reaches it all; _agent_cg_release
│   │                  ends what an agent left; agent_cgroups_tick removes the emptied cgroups,
│   │                  with any made inside them
│   └── _agent_start_agnos   (2.4.0) spawn_path#43 with a channel endowed (CH_ENDOW); no limits
│                            (agnos has none: audited, limits_enforced false)
│
├── supervisor.cyr   Health monitoring and quotas
│   ├── CircuitBreaker     Closed → Open → HalfOpen state machine
│   ├── OutputCapture      Ring buffer for stdout/stderr
│   ├── ResourceQuota      Memory/CPU limits, applied as an agent's rlimits at start (1 GiB, 3600 s
│   │                      by default), and warning / kill thresholds
│   └── AgentHealth        Per-agent health tracking
├── sched.cyr        Task start / complete over samay (2.3.1)
│   └── sched_task_start / sched_task_complete   SCHEDULED → RUNNING → COMPLETED/FAILED, capacity returned
│
├── memory.cyr       Per-agent key-value store. No route reaches it until agent identity (roadmap 2.5.x)
│   ├── AgentMemoryStore   Filesystem-backed, atomic write (temp + fsync + rename)
│   ├── validate_key       Path traversal prevention
│   └── sanitize_key       Filename-safe transformation
├── vector_store.cyr Embedded vector search
│   ├── cosine_similarity  f64 dot product / magnitude
│   ├── VectorIndex        Cosine search, keeping only the best top_k while scoring (2.2.3)
│   └── VectorEntry        id, embedding, content, metadata
├── rag.cyr          Retrieval-augmented generation
│   ├── chunk_text         Overlapping text chunking
│   ├── tokenize           Alphanumeric lowercase tokenizer
│   ├── rag_ingest_text    Chunk → hash-embed → index
│   └── rag_query_text     Embed → search → format context
│
├── mcp.cyr          MCP registries: tools, and (2.1.0) resources and prompts
│   ├── McpHostRegistry    Builtin + external maps; an external registration never takes a builtin's name
│   ├── mcp_manifest_json  GET /v1/mcp/tools
│   └── validate_callback_url   http:// or https:// only (a scheme check, not a host allowlist)
├── mcp_builtin.cyr  The 13 builtin tools, dispatched in-process
│   ├── mcp_libro_init     bote's five libro tools, bound to daimon's audit chain
│   ├── mcp_web_init       bote's web_fetch / web_search (these run in a detached child)
│   ├── mcp_nein_init      nein's six firewall tools, behind daimon_nein_gate: nein_allow / nein_deny
│   │                      refused until caller authentication (roadmap 2.5.x)
│   └── mcp_dispatch_builtin   name → handler
│
├── screen.cyr       Capture management (no route reaches it)
│   ├── CapturePermissionManager   Per-agent permissions + rate limiting
│   └── RecordingManager   Session lifecycle (active/paused/stopped)
├── federation.cyr   Multi-node clustering (no route reaches it; federation_nodes in /v1/metrics stays 0)
│   ├── FederationNode     Node with Raft role + capabilities
│   ├── FederationManager  Heartbeat health, election, step-down
│   ├── fed_score_node     4-factor weighted placement (resource/locality/load/affinity)
│   └── fed_place_agent    Best-node selection
├── fed_vector_store.cyr   FederatedVectorStore — collection replicas, cross-node merge + dedup
├── edge.cyr         Edge fleet management
│   ├── EdgeNode           Status, capabilities, GPU inventory
│   ├── EdgeFleetManager   Register (with validation), heartbeat, health (evaluated on read), decommission
│   └── edge_fleet_stats   Aggregation across fleet
│
├── ipc.cyr          Messages, the message bus, RPC routing, and agent channels
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
├── http.cyr         HTTP plumbing
│   ├── http_parse_*       Method, path, query params, body
│   ├── http_host_allowed / http_origin_foreign   (2.3.4) the Host allowlist and the cross-site-write rule
│   ├── http_body_json + http_json_str/_int/_has/_text   (2.3.2) request bodies: the typed parser,
│   │                      strings decoded, one JSON object, no duplicate keys or U+0000
│   └── http_send_response   sandhi's sender on Linux; on agnos daimon_write_all's pieces (2.4.1)
├── api.cyr          Service-level endpoints (health, metrics)
├── api_agent.cyr    Agent lifecycle, message and output endpoints
├── api_mcp.cyr      MCP endpoints: tools, resources, prompts, calls (forwards run in a child)
├── api_rag.cyr      RAG ingest/query endpoints
├── api_edge.cyr     Edge fleet endpoints
├── api_sched.cyr    Scheduler endpoints, incl. task start/complete + a node's work list
├── router.cyr       http_route — 44 method + path routes; agent and task control refuse Origin (403)
└── server.cyr       Server lifecycle
    ├── rate_check         120 requests a minute per IP, a fixed window; one shared bucket for
    │                      peers with no address (every client on agnos)
    ├── handle_request     Host allowlist + cross-site-write refusal (VULN-012), then the router
    ├── server_bind_addr   config listen_addr (127.0.0.1 unless serve --listen)
    ├── server_loop        (2.3.4) daimon's own event loop: one thread, one epoll set over the
    │                      listener, connections (non-blocking reads) and agent channels; a tick
    │                      for deadlines, deferred answers (server_defer) and agents_tick.
    │                      AGNOS (2.4.0): the same loop, polled, yielding with pause#14
    ├── _srv_slot_for_new  (2.4.2) a free slot, or the oldest request still arriving gives way
    │                      (1 s old at least) when every slot is taken; http_evicted in /v1/metrics
    └── server_detach      (2.3.4) a handler's work in a child process (MCP forwards, web_fetch);
                           the loop relays its answer, 504 after 60 s. AGNOS (2.4.1): fork#96 and
                           a pipe; the child writes its answer whole (http_answer_into_pipe)
```

## Data Flow

```
Client (HTTP)
  │
  ▼
epoll → accept → read (non-blocking) → complete? → smuggling checks → Rate / Host / Origin → Route
  │                                         │
  ├─ /v1/agents ────────► AgentHandle map ──┤
  ├─ /v1/mcp/* ─────────► McpHostRegistry ──┤── builtin: in-process · external: a child forwards
  ├─ /v1/rag/* ─────────► RagPipeline ──────┤
  ├─ /v1/edge/* ────────► EdgeFleetManager ─┤
  ├─ /v1/scheduler/* ──► TaskScheduler ─────┤   (samay)
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
  ├─ fork/exec with RLIMIT_AS + RLIMIT_CPU, in its own process group (and cgroup, where daimon may)
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
loop polls and yields with `pause`. Since 2.4.1 its deadlines read `daimon_now_ms`, which survives a
refused TSC calibration. Its answers are written 512 bytes at a time, 1 ms apart, because a write a
local process has no room for stops the machine there.

## Consumers

Every AGNOS agent (over the HTTP API and, once started by daimon, its channel on fd 3), hoosh, agnoshi, aethersafha, and any consumer app that talks to the HTTP API.

## Key Design Decisions

1. **Single compilation unit, multi-file source** — `src/main.cyr` `include`s 29 per-domain `src/*.cyr` modules. Cyrius flattens the includes into one global scope and compiles in one pass; no separate library crate. The 1.2.8 split kept the monolith's source order; later modules came with their features (`mcp_builtin.cyr` / `audit.cyr` at 1.3.0, `secmem.cyr` at 1.3.2, `trace.cyr` at 1.3.3, `syscalls.cyr` at 2.1.6, `sched.cyr` at 2.3.1). At 2.4.3 `src/` is 10,044 lines; the largest modules are `agent.cyr` (1,668), `server.cyr` (1,211) and `ipc.cyr` (1,086).
2. **daimon's own event loop, sandhi's HTTP** (2.3.4, ADR-006) — `server_loop` owns accept and reads (one thread, epoll). Requests are framed, smuggling-checked and answered with sandhi's public functions, as sandhi's own loops did through 2.3.3. `serve --async` is accepted and runs the same loop. On AGNOS (2.4.0, ADR-007) the loop polls instead of waiting on epoll, and yields with `pause`. Single trust domain.
3. **Bump allocator** — fast allocation, no individual free. Single trust domain (see VULN-007 security gate for multi-tenant). So upkeep on the loop's tick must not allocate: `agents_tick` allocates nothing, idle or with a live agent (measured at 2.4.2).
4. **Everything is i64** — Cyrius type system. Structs are manually laid out with `alloc()` + `store64()`/`load64()` at fixed offsets.
5. **pidfd for signals** — race-free process management on Linux 5.3+, with `kill()` fallback (`daimon_signal`).
6. **Dependencies** — a Cyrius stdlib subset (`[deps].stdlib` in `cyrius.cyml`) and nine first-party git dependencies, all pinned by tag in `cyrius.cyml`: sakshi (logging, tracing), bote (MCP: libro and web tools), libro (the audit chain), majra (bote's event sink), sigil (crypto), bayan (JSON), samay (the scheduler), nein (firewall tools) and ai-hwaccel (samay's). sandhi (HTTP framing and answers, and the MCP client) comes with the stdlib. `cyrius deps` vendors all of it into `lib/`, which is gitignored; `cyrius.lock` records a hash for each of its 83 entries, and CI verifies them. `CLAUDE.md` says why each dep is there and what it pins.
