# Daimon

**Daimon** (Greek: δαίμων — guiding spirit) — AGNOS agent orchestrator.

The core runtime for the AGNOS ecosystem: agent lifecycle, HTTP API (port 8090), process supervision, agent IPC (a channel per agent, open as its fd 3), task scheduling, multi-node federation, edge fleet management, memory/vector/RAG stores, MCP tool dispatch (with a built-in libro audit trail), and screen capture. Written in [Cyrius](https://github.com/MacCracken/cyrius), ported from Rust.

## Building

Requires [Cyrius](https://github.com/MacCracken/cyrius) 6.6.6+ (pinned in `cyrius.cyml`; the committed `cyrius.lock` carries the 6.6.6 `cyrius` trailer).

```bash
cyrius lib sync                        # vendor stdlib subset from the pin (lib/ is gitignored)
cyrius deps                            # resolve git deps (sakshi, bote, libro, majra) into lib/
cyrius build src/main.cyr build/daimon # build
./build/daimon serve                   # start server on 127.0.0.1:8090 (--listen ADDR to change)
./build/daimon help                    # every flag (--agents-dir, --agent-env, --agent-output, ...)
```

## Testing

```bash
cyrius tests                        # every suite in tests/ — 1050 assertions, 17 files
cyrius fuzz                         # 7 property-based harnesses, 176,349 generated cases
cyrius bench tests/daimon.bcyr      # 27 benchmarks (+2 in tests/rag_ingest.bcyr)
sh tests/smoke.sh                   # the linked binary over HTTP: libro, tracing, regressions,
                                    # the agent lifecycle, the bind address, task start/complete,
                                    # request-string decoding, agent channels and messages,
                                    # the event loop, hosts and origins, captured output,
                                    # detached MCP calls
sh tests/test.sh                    # all of the above except the benchmarks
sh tests/agnos/run.sh --release     # on AGNOS: boots the released agnos kernel under QEMU and runs
                                    # daimon's agent lifecycle, channels and API there (92 checks;
                                    # CI runs it on every push). Without --release: ../agnos's build
```

**On AGNOS** (2.4.0) daimon starts, stops and hears its agents with the kernel's own primitives, and
(2.4.1) runs its calls to other servers in a child as on Linux.
What agnos does not do yet, such as resource limits, ending a process, and a loopback-only listener,
is filed with agnos. What daimon does meanwhile is in
[ADR-007](docs/adr/007-daimon-on-agnos.md) and the
[AGNOS audit](docs/audit/2026-09-23-agnos-platform-audit.md).

Every suite, benchmark and fuzz harness includes the real `src/` module it tests — none carries a
copy of daimon's code (that was the 2.2.x test-integrity arc; see the CHANGELOG for what it found).

## MCP audit tools

daimon hosts five built-in MCP tools over a hash-linked [libro](https://github.com/MacCracken/libro) audit chain — `libro_query`, `libro_verify`, `libro_export`, `libro_proof`, `libro_retention` — fed by daimon's own lifecycle and security events (agent spawn/stop, agent channels closed on a bad frame, rate-limit and SSRF-guard rejections, external MCP calls). List them at `GET /v1/mcp/tools`; invoke via `POST /v1/mcp/call`.

## Documentation

- [Quickstart](docs/guides/quickstart.md) · [API reference](docs/guides/api.md) · [Agent IPC](docs/guides/agent-ipc.md) · [Architecture](docs/architecture/overview.md) · [Roadmap](docs/development/roadmap.md)
- [CHANGELOG](CHANGELOG.md) · [Security policy](SECURITY.md) · [Contributing](CONTRIBUTING.md)

## Status

Ported from Rust (9,724 LOC → 4,141 LOC Cyrius at the port; 6,528 lines of `src/` at 2.3.1). The binary is 3.2 MB, statically linked (2.3.1). About half of that, 1.6 MB, is unreachable code the 6.x toolchain NOPs in place rather than removing — see CHANGELOG 1.2.4. The 181 KB this line used to give was the v1.0.1 port-era build (cyrius 4.2.0, BENCHMARKS.md).

## License

GPL-3.0-only
