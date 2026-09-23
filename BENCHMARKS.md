# Benchmarks — Rust v0.6.0 vs Cyrius v1.0.1

- **Rust**: v0.6.0, rustc 1.89, criterion, x86_64. Final benchmark run before port.
- **Cyrius**: v1.0.1, cyrius 4.2.0 (cc3 compiler, single-pass, no LLVM), lib/bench.cyr, `tests/daimon.bcyr`. Same machine.

> The Rust-vs-Cyrius tables below are the **frozen v1.0.1 port-era snapshot** (cyrius 4.2.0). The current toolchain baseline is captured separately just below.

## Current baseline — daimon 2.2.3 / cyrius 6.6.6 (2026-09-22) — THE REAL CODE

`./scripts/bench-history.sh` → `tests/daimon.bcyr`. Averages over the iteration counts shown; no
microbenchmark touches HTTP.

⛔ **Until 2.2.3 `tests/daimon.bcyr` benchmarked simplified local COPIES of daimon's functions, not
daimon.** At 2.2.1 `cmp` showed its binary was byte-identical before and after that release: it
could not observe any change to `src/`. Every benchmark now calls the shipping function, so the
seven marked † below measure different — larger — work than their pre-2.2.3 numbers did, and
those numbers are not comparable.

| Benchmark | avg | min | iters | pre-2.2.3 (copy) |
|---|---:|---:|---:|---:|
| config_default | 91ns | 88ns | 10000 | 91ns |
| cosine_128d | 616ns | 611ns | 10000 | 650ns |
| vector_insert_100x128d † | 173.0µs | 166.4µs | 100 | 26.0µs — filled arrays, never called `vindex_insert` |
| vector_search_1k_64d † | 377.3µs | 368.6µs | 100 | 316.1µs — scored, never ranked |
| rag_chunk_5k_chars (was rag_ingest_5k_chars) | 4.20µs | 4.10µs | 1000 | 4.14µs — same work; renamed because it never ingested |
| scheduler_100_tasks † | 318.4µs | 308.2µs | 100 | 88.4µs — counted fake structs, not samay |
| supervisor_register_1000 † | 2.437ms | 2.437ms | 10 | 356.2µs — one map_set, the real call fills three |
| mcp_register_100_tools | 70.3µs | 68.2µs | 100 | 56.2µs |
| mcp_manifest_100_tools † | 139.1µs | 133.0µs | 1000 | 65.5µs — sorted keys, built no JSON |
| mcp_find_tool_in_100 | 93ns | 92ns | 100000 | 142ns |
| mcp_extract_input_schema | 4.49µs | 4.28µs | 10000 | 5.02µs |
| edge_register_100 † | 898.5µs | 840.1µs | 100 | 85.2µs — no duplicate-name scan |
| edge_heartbeat_100 | 142.2µs | 140.1µs | 1000 | 221.5µs |
| edge_stats_500 | 56.6µs | 54.7µs | 1000 | 51.2µs |
| circuit_breaker_cycle † | 4.06µs | 3.99µs | 100000 | 1.09µs — no clock read, no log call |
| hashmap_1000_insert_lookup | 477.1µs | 460.1µs | 100 | 480.2µs |
| json_parse | 421ns | 409ns | 10000 | 418ns |
| secure_zero_4k | 5.46µs | 5.24µs | 100000 | 5.45µs |
| trace_id_hex | 47ns | 46ns | 100000 | 43ns |

`tests/rag_ingest.bcyr` (real since 2.1.6): `rag_ingest_real_5k` 133.1µs, `rag_chunk_only_5k` 508ns.

### 2.3.2 — request bodies

| Benchmark | avg | min | iters |
|---|---:|---:|---:|
| http_body_read3 — typed parse, boundary checks, three decoded fields | 1.31 µs | 1.30 µs | 10000 |
| json_parse — bayan's flat parse alone, same body (the pre-2.3.2 read; baseline only) | 0.46 µs | 0.46 µs | 10000 |

Reading a body correctly costs under a microsecond more per request. The committed 2.3.1 bench
binary and the 2.3.2 one were run back to back, three runs each, compared by median. All 23
benchmarks they share agree within −5.0% … +6.2%. The two largest moves are `trace_id_hex` (+3 ns)
and `mcp_register_100_tools` (+5.3%), on code 2.3.2 did not touch.

### 2.3.1 — task start / complete

| Benchmark | avg | min | iters |
|---|---:|---:|---:|
| sched_start_complete — start + complete one task, the node's capacity returned (+ a reset to Scheduled) | 3.49 µs | 3.23 µs | 100000 |

No regression. The committed 2.3.0 bench binary and the 2.3.1 one were run back to back, three runs
each, compared by median. All 22 benchmarks they share agree within −5.0% … +3.5%, and none of the
code 2.3.1 changed is on their path.

### 2.3.0 — the agent lifecycle

Three benchmarks for the code 2.3.0 put on the API. Every agent route runs the reap sweep first,
and the start route runs the spawn.

| Benchmark | avg | min | iters |
|---|---:|---:|---:|
| agent_spawn_reap — fork, child setup, exec `/bin/true`, exec report, reap | 1.35 ms | 1.28 ms | 200 |
| agent_reap_sweep_100_idle — 100 agents with no process (a load and a compare each) | 0.95 µs | 0.91 µs | 100000 |
| agent_reap_live — per RUNNING agent: one non-blocking `waitpid` | 0.51 µs | 0.48 µs | 100000 |

At the 1000-agent limit with every agent running, the sweep adds about 0.5 ms to an agent request.
It walks the registry with `map_iter`, so it allocates nothing per request.

**No regression in the 19 existing benchmarks.** The committed 2.2.3 bench binary and the 2.3.0 one
were run back to back on the same machine, three runs each; the table gives the medians. Every
benchmark agrees within −4.7% … +2.9%, and 16 of the 19 are faster. The absolute numbers sit a few
percent above the 2.2.3 table above for **both** binaries, so that difference is the machine's
state, not the code. The committed 2.2.3 binary itself read `mcp_find_tool_in_100` at 102 ns
against its recorded 93 ns.

| Benchmark | 2.2.3 | 2.3.0 | Δ |
|---|---:|---:|---:|
| config_default | 101ns | 100ns | −1.0% |
| cosine_128d | 679ns | 677ns | −0.3% |
| vector_insert_100x128d | 173.4µs | 172.8µs | −0.4% |
| vector_search_1k_64d | 412.1µs | 396.8µs | −3.7% |
| rag_chunk_5k_chars | 4.60µs | 4.45µs | −3.3% |
| scheduler_100_tasks | 338.9µs | 334.5µs | −1.3% |
| supervisor_register_1000 | 2.569ms | 2.517ms | −2.0% |
| mcp_register_100_tools | 69.1µs | 65.8µs | −4.7% |
| mcp_manifest_100_tools | 150.8µs | 146.5µs | −2.8% |
| mcp_find_tool_in_100 | 102ns | 105ns | +2.9% |
| mcp_extract_input_schema | 4.88µs | 4.69µs | −4.1% |
| edge_register_100 | 959.4µs | 940.9µs | −1.9% |
| edge_heartbeat_100 | 146.9µs | 145.8µs | −0.8% |
| edge_stats_500 | 60.2µs | 58.3µs | −3.1% |
| circuit_breaker_cycle | 4.20µs | 4.21µs | +0.2% |
| hashmap_1000_insert_lookup | 525.1µs | 510.4µs | −2.8% |
| json_parse | 453ns | 455ns | +0.4% |
| secure_zero_4k | 5.91µs | 5.72µs | −3.2% |
| trace_id_hex | 50ns | 48ns | −4.0% |

### The search fix these numbers exposed (2.2.3)

Run against the real function for the first time, `vector_search_1k_64d` read **7.51 ms**, not the
copy's 316 µs: `vindex_search` insertion-sorted every entry to return the top five, which is
quadratic in the index size on `POST /v1/rag/query`. It now keeps only the best `top_k` while
scoring — same results, order included (a differential test over 3,000 random indexes with
36,454 tied pairs found no difference). Per query, 64-d, top_k 5:

| entries | before | after |
|---:|---:|---:|
| 1,000 | 7.48 ms | 0.38 ms |
| 4,000 | 115 ms | 1.51 ms |
| 10,000 | 716 ms | 3.86 ms |

## Port-era comparison (frozen, v1.0.1)

⛔ **Correction (2.2.3).** Every Cyrius number in the frozen tables below was measured against the
benchmark file's local COPIES, not daimon's source (see the note above), so where the copy did less
work than the Rust benchmark the comparison flatters Cyrius. The two recorded "wins" were artifacts
of exactly that: `rag_ingest_5k_chars` timed `chunk_text` alone (a real 5 KB ingest is 133 µs against
Rust's 210 µs — still ahead, by 1.6x, not 42x), and `vector_insert_100x128d` timed array fills (a
real 100-entry insert is 173 µs against Rust's 343 µs — 2.0x, not 3.5x). The tables are kept as the
historical record; read their Cyrius column as a lower bound.

## Core Operations

| Benchmark | Rust (ns) | Cyrius (ns) | Ratio | Winner |
|---|---:|---:|---|---|
| config_default | 21 | 532 | 25x | Rust |
| cosine_similarity_128d | 101 | 1,000 | 10x | Rust |
| vector_insert_100x128d | 343,362 | 97,000 | 3.5x | **Cyrius** |
| vector_search_1k_64d_top10 | 68,706 | 507,000 | 7.4x | Rust |
| rag_ingest_5k_chars | 209,955 | 5,000 | 42x | **Cyrius** |
| rag_query_50_docs | 30,447 | — | — | — |
| scheduler_100_tasks_10_nodes | 74,834 | 111,000 | 1.5x | Rust |

## Agent Registration

| Benchmark | Rust (ns) | Cyrius (ns) | Ratio | Winner |
|---|---:|---:|---|---|
| supervisor_register_1000 | 204,971 | 512,000 | 2.5x | Rust |

## MCP Dispatch

| Benchmark | Rust (ns) | Cyrius (ns) | Ratio | Winner |
|---|---:|---:|---|---|
| mcp_register_100_tools | 51,916 | 79,000 | 1.5x | Rust |
| mcp_manifest_100_tools | 22,104 | 102,000 | 4.6x | Rust |
| mcp_find_tool_in_100 | 16 | 538 | 34x | Rust |

## Edge Fleet

| Benchmark | Rust (ns) | Cyrius (ns) | Ratio | Winner |
|---|---:|---:|---|---|
| edge_register_100_nodes | 69,314 | 114,000 | 1.6x | Rust |
| edge_heartbeat_100_nodes | 8,267 | 247,000 | 30x | Rust |
| edge_stats_500_nodes | 765 | 46,000 | 60x | Rust |
| federation_score_100_nodes | 1,678 | — | — | — |

## API Latency (Rust only — in-process axum, no network)

| Benchmark | Rust (ns) |
|---|---:|
| api_health | 53,066 |
| api_mcp_tools_list | 53,518 |
| api_edge_stats | 53,914 |
| api_metrics | 54,056 |

## Cyrius-Only Benchmarks (no Rust equivalent)

| Benchmark | Cyrius (ns) |
|---|---:|
| circuit_breaker_cycle | 1,000 |
| hashmap_1000_insert_lookup | 604,000 |
| json_parse | 916 |

## Analysis

### Where Cyrius wins

**Allocation-heavy workloads** — `vector_insert` and `rag_ingest` (recorded as 3.5x and 42x; **2.0x
and 1.6x** against the real code — see the 2.2.3 correction above). The bump allocator eliminates malloc/free overhead entirely. Insert 100 vectors with 128-dimension embeddings: Rust spends time in HashMap resizing and Vec allocation; Cyrius just increments a pointer.

### Where Rust wins

**Tight compute loops** — cosine similarity (10x), MCP find (34x). LLVM applies SIMD vectorization, loop unrolling, and branch prediction hints that a single-pass compiler cannot. The 101 ns Rust cosine vs 1,000 ns Cyrius cosine reflects SSE/AVX auto-vectorization on the dot product.

**HashMap iteration** — edge heartbeat (30x), edge stats (60x). Rust's HashMap iterates values directly with a flat memory layout. Cyrius hashmap iterates via `map_keys()` → `vec_get()` → `map_get()` per entry — three indirections per iteration. This is the largest optimization opportunity in the Cyrius stdlib.

### Parity zone (1-3x)

Scheduler scheduling (1.5x), supervisor registration (2.5x), MCP registration (1.5x), edge registration (1.6x). These are dominated by hashmap insert/lookup where both implementations use similar algorithms (open-addressing with FNV-1a in Cyrius, Robin Hood in Rust).

### Optimization opportunities

1. **Hashmap value iteration** — add `map_values()` or `map_for_each()` to cyrius stdlib to avoid the keys→get indirection chain. Would close the 30-60x gap on edge heartbeat/stats.
2. **SIMD cosine** — hand-written SSE2 `asm {}` block for the dot product inner loop. Would close the 10x gap.
3. **Inline sort** — ✅ vector search, 2.2.3: top-k selection replaced the full insertion sort (see
   the table above). Scheduling's sort is samay's now.

## Project Comparison

### Size

| Metric | Rust v0.6.0 | Cyrius v1.0.1 | Change |
|---|---:|---:|---|
| Language | rustc 1.89 | cyrius 4.2.0 | — |
| Source LOC | 9,724 | 4,141 | −57% |
| Test LOC | 611 (integration) | 1,837 | +200% |
| Benchmark LOC | 452 | 516 | +14% |
| Fuzz LOC | 0 | 554 | new |
| Binary (default) | 4.0 MB | 181 KB | **−96%** |
| Binary (full features) | 8.2 MB | 181 KB | **−98%** |
| Dependencies | 193 crates | 0 external | **−100%** |
| Stdlib modules | — | 17 | — |
| Build cache | 16 GB (target/) | ~0 | — |

### Correctness

| Module | Rust Status | Cyrius Status | Notes |
|---|---|---|---|
| error | Complete | Complete | Enum codes + HTTP status mapping |
| config | Complete | Complete | Defaults, accessors |
| agent | Complete | Complete (2.3.0) | On the API since 2.3.0 (start / stop / pause / resume / delete, reaping). Its process code first ran at 2.3.0 and eight defects were fixed then (CHANGELOG) |
| supervisor | Complete | Complete | Circuit breaker, output capture, health, quotas |
| memory | Complete | Complete (2.2.3) | ⛔ Listed "Complete" since the port and never run: the first call of any kind killed the process until 2.2.3. No route reaches it yet; no `tags` field, so `list_by_tag` is a substring search |
| vector_store | Complete | Complete | Cosine similarity, search, normalize |
| rag | Complete | Complete | Chunk, embed, ingest, query, context format |
| mcp | Complete | Complete | Registry + types; external forwarding via sandhi_rpc_mcp_call (1.2.1); bote libro-tool re-exports being wired (1.3.0) |
| screen | Complete | Complete | Permissions, rate limiting, recording sessions |
| scheduler | Complete | Complete (2.3.1) | samay since 2.0.0. Tasks can start and complete through the API since 2.3.1 (`src/sched.cyr`); before that every task stopped at Scheduled |
| federation | Complete | Complete | Cluster, election, scoring, placement, vector store |
| edge | Complete | Complete | Register, heartbeat, health, decommission, stats |
| ipc | Complete | Partial | Message bus + RPC registry tested; the Unix-socket half first ran at 2.2.3 (`agent_ipc_new` crashed, `agent_ipc_bind` made a directory at the socket path). No route wires it yet (the last step of roadmap 2.3.x) |
| api | Complete | Complete | 41 method + path routes (the Rust original had no agent or task control; 2.3.0 added 5, 2.3.1 added 3) |
| logging | Complete | Complete | sakshi integration |
| firewall | Complete | Integrated (2.1.8) | nein's MCP tools; the mutating half is gated shut until caller authentication (roadmap 2.5.x) |
| http-forward | Complete | Complete | External MCP forwarding via sandhi_rpc_mcp_call (1.2.1) |

### Test Coverage

| | Rust | Cyrius |
|---|---|---|
| Unit tests | 305 | — (inline in test groups) |
| Integration tests | 28 | 833 assertions / 17 suites, each against its real `src/` module (2.3.2) |
| Benchmarks | 19 | 26, against the real code (2.3.2) |
| Fuzz harnesses | 0 | 7, property-based, run in CI (2.3.2) |
| HTTP smoke | — | tests/smoke.sh, 70 checks, run in CI (2.3.2) |
| Security audit | — | 17 findings (2026-04-13: 10; 2026-09-22 lifecycle: 7) — see docs/audit/ |
