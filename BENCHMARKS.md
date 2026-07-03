# Benchmarks — Rust v0.6.0 vs Cyrius v1.0.1

- **Rust**: v0.6.0, rustc 1.89, criterion, x86_64. Final benchmark run before port.
- **Cyrius**: v1.0.1, cyrius 4.2.0 (cc3 compiler, single-pass, no LLVM), lib/bench.cyr, `tests/daimon.bcyr`. Same machine.

> The Rust-vs-Cyrius tables below are the **frozen v1.0.1 port-era snapshot** (cyrius 4.2.0). The current toolchain baseline is captured separately just below.

## Current baseline — daimon 1.3.0 / cyrius 6.3.43 (2026-07-03)

Re-baselined under the 6.3.43 toolchain (`./scripts/bench-history.sh` → `tests/daimon.bcyr`). Averages over the iteration counts shown; no microbenchmark touches HTTP, so these are pure in-memory op costs.

| Benchmark | avg | min | iters |
|---|---:|---:|---:|
| config_default | 1.510µs | 907ns | 10000 |
| cosine_128d | 2.068µs | 908ns | 10000 |
| vector_insert_100x128d | 58.548µs | 53.917µs | 100 |
| vector_search_1k_64d | 325.916µs | 316.032µs | 100 |
| rag_ingest_5k_chars | 7.629µs | 5.657µs | 1000 |
| scheduler_100_tasks | 194.797µs | 189.339µs | 100 |
| supervisor_register_1000 | 399.205µs | 382.590µs | 10 |
| mcp_register_100_tools | 70.228µs | 67.257µs | 100 |
| mcp_manifest_100_tools | 69.505µs | 66.349µs | 1000 |
| mcp_find_tool_in_100 | 1.456µs | 907ns | 100000 |
| mcp_extract_input_schema | 2.216µs | 908ns | 10000 |
| edge_register_100 | 197.444µs | 190.877µs | 100 |
| edge_heartbeat_100 | 721.560µs | 668.032µs | 1000 |
| edge_stats_500 | 49.119µs | 45.816µs | 1000 |
| circuit_breaker_cycle | 5.372µs | 4.260µs | 100000 |
| hashmap_1000_insert_lookup | 489.127µs | 471.499µs | 100 |
| json_parse | 1.976µs | 908ns | 10000 |

The stdlib hot-path shapes are unchanged from the port-era analysis below (hashmap value-iteration and compute-loop vectorization remain the standing optimization opportunities).

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

**Allocation-heavy workloads** — `vector_insert` (3.5x faster) and `rag_ingest` (42x faster). The bump allocator eliminates malloc/free overhead entirely. Insert 100 vectors with 128-dimension embeddings: Rust spends time in HashMap resizing and Vec allocation; Cyrius just increments a pointer.

### Where Rust wins

**Tight compute loops** — cosine similarity (10x), MCP find (34x). LLVM applies SIMD vectorization, loop unrolling, and branch prediction hints that a single-pass compiler cannot. The 101 ns Rust cosine vs 1,000 ns Cyrius cosine reflects SSE/AVX auto-vectorization on the dot product.

**HashMap iteration** — edge heartbeat (30x), edge stats (60x). Rust's HashMap iterates values directly with a flat memory layout. Cyrius hashmap iterates via `map_keys()` → `vec_get()` → `map_get()` per entry — three indirections per iteration. This is the largest optimization opportunity in the Cyrius stdlib.

### Parity zone (1-3x)

Scheduler scheduling (1.5x), supervisor registration (2.5x), MCP registration (1.5x), edge registration (1.6x). These are dominated by hashmap insert/lookup where both implementations use similar algorithms (open-addressing with FNV-1a in Cyrius, Robin Hood in Rust).

### Optimization opportunities

1. **Hashmap value iteration** — add `map_values()` or `map_for_each()` to cyrius stdlib to avoid the keys→get indirection chain. Would close the 30-60x gap on edge heartbeat/stats.
2. **SIMD cosine** — hand-written SSE2 `asm {}` block for the dot product inner loop. Would close the 10x gap.
3. **Inline sort** — the insertion sort in vector search and scheduler scheduling could be replaced with a more cache-friendly merge sort for larger datasets.

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
| agent | Complete | Complete | Lifecycle, /proc, pidfd signals, rlimits |
| supervisor | Complete | Complete | Circuit breaker, output capture, health, quotas |
| memory | Complete | Complete | CRUD, list_keys, list_by_tag, clear, usage_bytes |
| vector_store | Complete | Complete | Cosine similarity, search, normalize |
| rag | Complete | Complete | Chunk, embed, ingest, query, context format |
| mcp | Complete | Complete | Registry + types; external forwarding via sandhi_rpc_mcp_call (1.2.1); bote libro-tool re-exports being wired (1.3.0) |
| screen | Complete | Complete | Permissions, rate limiting, recording sessions |
| scheduler | Complete | Complete | NodeCapacity, scheduling, cron, preemption, stats |
| federation | Complete | Complete | Cluster, election, scoring, placement, vector store |
| edge | Complete | Complete | Register, heartbeat, health, decommission, stats |
| ipc | Complete | Complete | Unix sockets, message bus, RPC registry |
| api | Complete | Complete | 24/24 endpoints |
| logging | Complete | Complete | sakshi integration |
| firewall | Complete | **Blocked** | Requires nein Cyrius port |
| http-forward | Complete | Complete | External MCP forwarding via sandhi_rpc_mcp_call (1.2.1) |

### Test Coverage

| | Rust | Cyrius |
|---|---|---|
| Unit tests | 305 | — (inline in test groups) |
| Integration tests | 28 | 225 assertions / 26 groups |
| Benchmarks | 19 | 17 |
| Fuzz harnesses | 0 | 5 |
| Security audit | — | 10 findings, 9 fixed, 1 gated |
