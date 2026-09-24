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

### 2.4.2 — 2.3.x's last items

**A contained start costs 0.25 ms more.** `agent_spawn_reap_contained` (new in 2.4.2) is
`agent_spawn_reap` through a cgroup of the agent's own: the `mkdir`, the child's move into it, the
check of where it is, and the `rmdir` after the reap. Five runs of `tests/daimon.bcyr` under
`systemd-run --user --scope`, both benchmarks in each run's process, at a load average under 2:

| Benchmark | median | the five runs |
|---|---:|---|
| agent_spawn_reap | 1.362 ms | 1.362, 1.362, 1.366, 1.424, 1.362 |
| agent_spawn_reap_contained | 1.616 ms | 1.616, 1.616, 1.592, 1.788, 1.587 |

Five runs earlier, at a load average of 13, gave 1.411 and 1.690 ms (0.28 ms). Where daimon cannot
make cgroups the contained benchmark is left out, and a note after the report says so.

**One client holding every slot.** Measured over HTTP: 128 connections, each sending one more byte
of an unfinished request every 2 s, then a new client's `/v1/health`. 2.4.1: 28.634 s. 2.4.2:
0.001 s, with one held connection closed (`http_evicted` 1) and the other 127 still open.
(`tests/smoke.sh` checks the same with a byte every 0.5 s.)

**The upkeep allocates nothing on a tick.** Measured, not benchmarked: the gap between two `alloc(8)`
across 1000 calls of `agents_tick` is 0 bytes, idle and with a live agent. A cgroup the queue cannot
remove costs its three walks (17,032 bytes over the first ten ticks, then nothing).

**The rest of Linux is unchanged.** 2.4.1 against 2.4.2, `tests/daimon.bcyr`, ten interleaved runs
with the order alternating, medians, at a load average of 1.7–2.0. Spread is (max − min) over the
median. On the benchmarked paths 2.4.2 changed the spawn (the cgroup it is handed, none here) and
the agent handle (120 → 136 bytes). Everything it reaches is within ±1%:

| Benchmark | 2.4.1 | 2.4.2 | change | spread 2.4.1 | spread 2.4.2 |
|---|---:|---:|---:|---:|---:|
| agent_spawn_reap | 1.446 ms | 1.433 ms | −0.9% | 7% | 8% |
| agent_reap_sweep_100_idle | 973.5 ns | 972.0 ns | −0.2% | 2% | 4% |
| agent_reap_live | 496.0 ns | 496.5 ns | +0.1% | 7% | 10% |
| ipc_frame_roundtrip | 12.01 µs | 11.99 µs | −0.2% | 5% | 8% |
| ipc_poll_100_idle | 9.386 µs | 9.383 µs | −0.0% | 5% | 5% |
| bus_broadcast_take_100 | 14.11 µs | 14.14 µs | +0.2% | 18% | 8% |
| config_default (untouched) | 84.5 ns | 92.0 ns | +8.9% | 8% | 10% |

The other benchmarks are within ±3.5%, except two:
- `mcp_find_tool_in_100` −15.8%, with spreads of 49% and 59%: noise.
- `config_default` +8.9%, in code 2.4.2 did not change: `src/config.cyr` differs only in its version
  string. An earlier A/B of twenty runs, on the tree before review, showed the same +8%, so it is
  not noise. Its cause was not measured: there is no `perf` on this machine.

A first A/B at a load average of 9–11 was discarded: its spreads were 12–251%, and untouched code
moved by as much as 39%.

### 2.4.1 — AGNOS follow-ups

**No performance claim for AGNOS.** On agnos, 2.4.1 writes each answer 1 KB at a time with a yield
between writes, because one send over a connection's 2 KB receive ring to a local process stops the
machine there. That trades speed for not stopping; measuring it under QEMU would measure QEMU.

**The Linux side is unchanged.** 2.4.0 against 2.4.1, `tests/daimon.bcyr`, ten interleaved runs with
the order alternating, medians. On Linux, 2.4.1 changed two things: the clock call (`daimon_now_ms`,
in the loop, the channels and the stops) and one compare in `http_send_response`. Everything they
reach is within ±2%:

| Benchmark | 2.4.0 | 2.4.1 | change |
|---|---:|---:|---:|
| agent_spawn_reap | 1.365 ms | 1.367 ms | +0.2% |
| ipc_frame_roundtrip | 12.80 µs | 12.62 µs | −1.4% |
| ipc_poll_100_idle | 10.17 µs | 10.11 µs | −0.6% |
| bus_broadcast_take_100 | 15.25 µs | 15.00 µs | −1.7% |
| http_body_read3 | 1.411 µs | 1.395 µs | −1.1% |

Four benchmarks in code 2.4.1 did not touch moved 3–7%:
- `cosine_128d` +5.4%;
- `mcp_register_100_tools` +6.7%;
- `edge_stats_500` +4.2%;
- `mcp_find_tool_in_100` +3.0%.

Their own run-to-run spread in the same set was 12–22% (max − min, over the median). Another session
was using the machine, so this is noise, not a change. The absolute numbers are higher than in
2.4.0's section for the same reason; only the within-set comparison counts. The binary grew 4.2 KB
(ai-hwaccel 2.3.27).

### 2.4.0 — daimon on AGNOS

**No performance claim for AGNOS yet.** 2.4.0 makes daimon's agent lifecycle, channels and loop
correct there (the guest test, 64 checks). Measuring them under QEMU would measure QEMU, and the
kernel's inbound accept is filed as broken.

**The Linux side is unchanged.** 2.3.4 against 2.4.0, `tests/daimon.bcyr`, six interleaved runs,
medians. Everything is within ±3% except two benchmarks in code 2.4.0 did not change (`src/mcp.cyr`
and `src/edge.cyr` are untouched): code layout, not code.

| Benchmark | 2.3.4 | 2.4.0 | change |
|---|---:|---:|---:|
| agent_spawn_reap | 1.457 ms | 1.427 ms | −2.1% |
| ipc_frame_roundtrip | 11.63 µs | 11.64 µs | +0.1% |
| bus_broadcast_take_100 | 13.85 µs | 13.69 µs | −1.1% |
| mcp_manifest_100_tools (untouched) | 141.18 µs | 145.60 µs | +3.1% |
| edge_stats_500 (untouched) | 59.16 µs | 61.57 µs | +4.1% |

The agent handle grew from 112 to 120 bytes (`limits_enforced`), and the channel record from 80 to
104 (the record buffer agnos needs).

### 2.3.4 — daimon's own event loop

**The allocation lock is gone.** 2.3.3's channel thread made every allocation take the stdlib's
heap lock once an agent had started. 2.3.4 starts no thread. The suite run with 100 channels open
(medians of three), against 2.3.3 with its thread running:

| Benchmark | 2.3.3, thread running | 2.3.4, 100 channels open |
|---|---:|---:|
| config_default | 294 ns | 87 ns |
| json_parse | 1.24 µs | 440 ns |
| rag_chunk_5k_chars | 11.87 µs | 4.39 µs |
| http_body_read3 | 2.75 µs | 1.25 µs |
| mcp_manifest_100_tools | 257.6 µs | 141.3 µs |
| hashmap_1000_insert_lookup | 737.2 µs | 497.0 µs |

**HTTP, sequential requests over fresh connections** (3000 × `GET /v1/health`, one client, three
interleaved runs, medians). Answered 200: each block of 100 requests comes from its own loopback
address (127.0.0.2 to .31), under the per-address limit of 120 a minute.

| | µs / request |
|---|---:|
| 2.3.3 (sandhi's sync loop) | 111.9 |
| 2.3.4 | 113.3 (+1.3%; higher in each of the three pairs) |

The loop was first compared with all 3000 requests from one address, which the rate limiter
answers 429 after the 120th. Each refusal also appends an audit entry. On that path the first
version of the loop measured 142.6 µs against 2.3.3's 133.7 (+6.7%). Reading on accept (a local
client has usually sent its whole request) removed an epoll round trip: 133.8 against 134.6.

A request beside a client that stalls mid-headers answered in 6 ms, where 2.3.3's loop blocked for up
to 5 s. During a 5 s stop of an agent that ignores SIGTERM, `/v1/health` took 0.001 s; in 2.3.3 it
took 4.8 s.

**Calls that wait on another server run in a child** (`server_detach`). A 2 s MCP endpoint, with
`/v1/health` sent 0.3 s into the call:

| path | health, 2.3.3 | health, 2.3.4 | the call, 2.3.3 | the call, 2.3.4 |
|---|---:|---:|---:|---:|
| `tools/call` (external) | 1.700 s | 0.0004 s | 2.001 s | 2.002 s |
| `resources/read` | 1.700 s | 0.0004 s | 2.001 s | 2.002 s |
| `web_fetch` | 1.699 s | 0.0005 s | 2.001 s | 2.002 s |

The cost is a fork per call. 300 sequential `tools/call` to an endpoint that answers at once, three
runs each, medians: 470.3 µs per call on 2.3.3, 616.8 µs detached (+146.5 µs). A first version
relayed the child's answer on the 10 ms tick and added 10 ms to every call; the pipe is now in the
epoll set.

**The loop uses epoll.** A poll-based version cost, per pass, what was open:

| open channels | 10 | 100 | 1000 |
|---|---:|---:|---:|
| µs per poll pass (`ipc_poll_once`, which tests still use) | 2.6 | 9.3 | 92.7 |

**Suite A/B**, 2.3.3 against 2.3.4 (`tests/daimon.bcyr`, three interleaved runs, medians). The shared
benchmarks agree within ±3%, except:
- `edge_register_100`: 930.0 → **282.2 µs**. The duplicate-name check is a lookup, not a scan
  over a key vector allocated per registration.
- `edge_stats_500`: 58.6 → 63.6 µs (+8.4%). It now also totals the nodes' capacities.
- `agent_reap_live`: 522 → 496 ns (−5%), on code 2.3.4 did not touch.

| New or changed benchmark | 2.3.4 |
|---|---:|
| ipc_frame_roundtrip — write, one pass of the channels, read the ACK (2.3.3, through the thread: 11.6 µs) | 12.1 µs |
| ipc_poll_100_idle — a poll pass over 100 quiet channels | 9.5 µs |
| bus_broadcast_take_100 — a broadcast published to 100 queues, then taken from each (the last take frees it) | 14.0 µs |

`ipc_poll_100_idle` replaces `ipc_drain_empty` (the hand-off it timed is gone), and
`bus_broadcast_take_100` replaces `bus_broadcast_100` (publish only). The count stays 27 + 2.

### 2.3.3 — agent channels

Medians of three runs (`tests/daimon.bcyr`; the broadcast A/B from one harness built against both
2.3.2 and 2.3.3):

| Benchmark | 2.3.2 | 2.3.3 | iters |
|---|---:|---:|---:|
| bus_broadcast_100 — publish to `*` with 100 subscribers (each queue emptied again) | 11.18 µs | **3.37 µs** | 10000 |
| ipc_frame_roundtrip — one message through an agent's channel: the agent's write, the service thread's read, parse and hand-off, the ACK, the agent's read of it, the main thread's take | — | 11.6 µs | 10000 |
| ipc_drain_empty — what every HTTP request now does first, with nothing pending | — | 56 ns | 100000 |

The broadcast walks the subscriber map in place (`map_iter`) instead of building a key vector for
each message. The other 24 shared benchmarks agree within −2.8% … +1.1%, except for two. On
`http_body_read3` (+4.0%), a focused re-run of five runs each gave 1.301 against 1.305 µs.
`mcp_find_tool_in_100` swung between 94 and 154 ns within each build.

⚠ **With the channel thread running**, every allocation takes the stdlib's heap lock. The thread
starts at the first agent start (ADR-005). Here is the same suite with the thread started first,
three runs each, medians:

| Benchmark | no thread | thread running | ratio |
|---|---:|---:|---:|
| alloc(64) (scratch harness) | 10 ns | 53 ns | 5.3× |
| config_default | 87 ns | 294 ns | 3.38× |
| json_parse | 429 ns | 1.24 µs | 2.88× |
| rag_chunk_5k_chars | 4.31 µs | 11.87 µs | 2.75× |
| http_body_read3 | 1.29 µs | 2.75 µs | 2.14× |
| mcp_extract_input_schema | 4.55 µs | 8.67 µs | 1.90× |
| mcp_register_100_tools | 59.2 µs | 112.0 µs | 1.89× |
| mcp_manifest_100_tools | 140.9 µs | 257.6 µs | 1.83× |
| hashmap_1000_insert_lookup | 483.4 µs | 737.2 µs | 1.52× |
| scheduler_100_tasks | 322.5 µs | 403.6 µs | 1.25× |
| vector_insert_100x128d / vector_search_1k_64d | 173.0 / 389.0 µs | 191.7 / 426.7 µs | 1.11× / 1.10× |
| agent_spawn_reap, agent_reap_*, sched_start_complete, circuit_breaker_cycle, secure_zero_4k, edge_heartbeat_100, bus_broadcast_100 | — | — | 0.98–1.01× |

The paths at the bottom allocate little or nothing. The roadmap records the ways to remove the cost.

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
| ipc | Complete | Complete (2.3.4) | Agents send on a channel (a socketpair, their fd 3), read by daimon's event loop; HTTP clients send and take messages; messages are freed, names first-wins (2.3.4). The socket-file endpoint was removed at 2.3.3 |
| api | Complete | Complete | 41 method + path routes (the Rust original had no agent or task control; 2.3.0 added 5, 2.3.1 added 3) |
| logging | Complete | Complete | sakshi integration |
| firewall | Complete | Integrated (2.1.8) | nein's MCP tools; the mutating half is gated shut until caller authentication (roadmap 2.5.x) |
| http-forward | Complete | Complete | External MCP forwarding via sandhi_rpc_mcp_call (1.2.1) |

### Test Coverage

| | Rust | Cyrius |
|---|---|---|
| Unit tests | 305 | — (inline in test groups) |
| Integration tests | 28 | 1087 assertions / 17 suites, each against its real `src/` module (2.4.2); AGNOS guest test 92 checks (tests/agnos/run.sh --release, in CI); aarch64 VM, agent 212 + portability 46 under a real kernel (tests/aarch64/run.sh, in CI) |
| Benchmarks | 19 | 29, against the real code (2.3.4) |
| Fuzz harnesses | 0 | 7, property-based, run in CI (2.3.2) |
| HTTP smoke | — | tests/smoke.sh, 131 checks, run in CI (2.4.0) |
| Security audit | — | 28 findings (2026-04-13: 10; 2026-09-22 lifecycle: 8; 2026-09-23 AGNOS platform: 10, filed with agnos) — see docs/audit/ |
