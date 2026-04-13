# Benchmarks — Rust vs Cyrius

Comparison of the final Rust (v0.6.0, criterion) and Cyrius (v0.7.0, lib/bench.cyr) benchmark runs.

## Core Operations

| Benchmark | Rust | Cyrius | Ratio |
|---|---|---|---|
| config_default | 21 ns | 535 ns | 25x slower |
| cosine_similarity_128d | 101 ns | 1,000 ns | 10x slower |
| vector_insert_100x128d | 343 us | 88 us | **3.9x faster** |
| vector_search_1k_64d_top10 | 69 us | 474 us | 6.9x slower |
| rag_ingest_5k_chars | 210 us | 4 us | **52x faster** |
| scheduler_100_tasks_10_nodes | 75 us | 110 us | 1.5x slower |

## Agent Registration

| Benchmark | Rust | Cyrius | Ratio |
|---|---|---|---|
| supervisor_register_1000 | 205 us | 514 us | 2.5x slower |

## MCP Dispatch

| Benchmark | Rust | Cyrius | Ratio |
|---|---|---|---|
| mcp_register_100_tools | 52 us | 80 us | 1.5x slower |
| mcp_manifest_100_tools | 22 us | 101 us | 4.6x slower |
| mcp_find_tool_in_100 | 16 ns | 523 ns | 33x slower |

## Edge Fleet

| Benchmark | Rust | Cyrius | Ratio |
|---|---|---|---|
| edge_register_100_nodes | 69 us | 112 us | 1.6x slower |
| edge_heartbeat_100_nodes | 8 us | 250 us | 31x slower |
| edge_stats_500_nodes | 765 ns | 47 us | 61x slower |

## Federation

| Benchmark | Rust | Cyrius | Ratio |
|---|---|---|---|
| federation_score_100_nodes | 1.7 us | _(not bench'd)_ | — |

## API Latency (Rust only — in-process axum, no network)

| Benchmark | Rust |
|---|---|
| api_health | 53 us |
| api_mcp_tools_list | 54 us |
| api_edge_stats | 54 us |
| api_metrics | 54 us |

## Additional Cyrius Benchmarks (no Rust equivalent)

| Benchmark | Cyrius |
|---|---|
| circuit_breaker_cycle | 1 us |
| hashmap_1000_insert_lookup | 612 us |
| json_parse | 947 ns |

## Analysis

**Cyrius wins** on allocation-heavy benchmarks (vector_insert, rag_ingest) due to the bump allocator — no malloc/free overhead, just pointer increment.

**Rust wins** on CPU-bound tight loops (cosine similarity, MCP find) due to LLVM optimizations (SIMD, branch prediction hints, inlining). The 10-33x gap on pure compute reflects the difference between an optimizing compiler and a single-pass direct-to-native compiler.

**Parity zone** (1-3x): scheduler scheduling, supervisor registration, MCP registration, edge registration — dominated by hashmap operations where both implementations use similar algorithms.

**Edge heartbeat/stats** gap (31-61x) is driven by Cyrius iterating map keys via `map_keys()` + `vec_get()` per node, while Rust's HashMap iterates values directly. This is an optimization opportunity for the cyrius hashmap stdlib.

## Binary Size

| | Rust (default) | Rust (full) | Cyrius |
|---|---|---|---|
| Binary | 4.0 MB | 8.2 MB | **162 KB** |
| Dependencies | 193 crates | 193 crates | 0 external |

## Test Coverage

| | Rust | Cyrius |
|---|---|---|
| Tests | 333 | 200 assertions / 26 groups |
| Benchmarks | 19 | 16 |
| Fuzz harnesses | 0 | 5 |
