# Daimon

**Daimon** (Greek: δαίμων — guiding spirit) — AGNOS agent orchestrator.

The core runtime for the AGNOS ecosystem: agent lifecycle, HTTP API (port 8090), process supervision, IPC over Unix sockets, task scheduling, multi-node federation, edge fleet management, memory/vector/RAG stores, MCP tool dispatch (with a built-in libro audit trail), and screen capture. Written in [Cyrius](https://github.com/MacCracken/cyrius), ported from Rust.

## Building

Requires [Cyrius](https://github.com/MacCracken/cyrius) 6.6.6+ (pinned in `cyrius.cyml`; the committed `cyrius.lock` carries the 6.6.6 `cyrius` trailer).

```bash
cyrius lib sync                        # vendor stdlib subset from the pin (lib/ is gitignored)
cyrius deps                            # resolve git deps (sakshi, bote, libro, majra) into lib/
cyrius build src/main.cyr build/daimon # build
./build/daimon serve                   # start server on port 8090
```

## Testing

```bash
cyrius tests                        # every suite in tests/ — 645 assertions, 16 files
cyrius fuzz                         # 6 property-based harnesses, ~160,000 generated cases
cyrius bench tests/daimon.bcyr      # 19 benchmarks (+2 in tests/rag_ingest.bcyr)
sh tests/smoke.sh                   # the linked binary over HTTP: libro, tracing, regressions
sh tests/test.sh                    # all of the above except the benchmarks
```

Every suite, benchmark and fuzz harness includes the real `src/` module it tests — none carries a
copy of daimon's code (that was the 2.2.x test-integrity arc; see the CHANGELOG for what it found).

## MCP audit tools

daimon hosts five built-in MCP tools over a hash-linked [libro](https://github.com/MacCracken/libro) audit chain — `libro_query`, `libro_verify`, `libro_export`, `libro_proof`, `libro_retention` — fed by daimon's own lifecycle and security events (agent spawn/stop, IPC auth denials, rate-limit and SSRF-guard rejections, external MCP calls). List them at `GET /v1/mcp/tools`; invoke via `POST /v1/mcp/call`.

## Documentation

- [Quickstart](docs/guides/quickstart.md) · [API reference](docs/guides/api.md) · [Architecture](docs/architecture/overview.md) · [Roadmap](docs/development/roadmap.md)
- [CHANGELOG](CHANGELOG.md) · [Security policy](SECURITY.md) · [Contributing](CONTRIBUTING.md)

## Status

Ported from Rust (9,724 LOC → 4,141 LOC Cyrius). 181 KB binary.

## License

GPL-3.0-only
