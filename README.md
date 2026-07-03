# Daimon

**Daimon** (Greek: δαίμων — guiding spirit) — AGNOS agent orchestrator.

The core runtime for the AGNOS ecosystem: agent lifecycle, HTTP API (port 8090), process supervision, IPC over Unix sockets, task scheduling, multi-node federation, edge fleet management, memory/vector/RAG stores, MCP tool dispatch (with a built-in libro audit trail), and screen capture. Written in [Cyrius](https://github.com/MacCracken/cyrius), ported from Rust.

## Building

Requires [Cyrius](https://github.com/MacCracken/cyrius) 6.3.43+ (pinned in `cyrius.cyml`).

```bash
cyrius lib sync                        # vendor stdlib subset from the pin (lib/ is gitignored)
cyrius deps                            # resolve git deps (sakshi, bote, libro, majra) into lib/
cyrius build src/main.cyr build/daimon # build
./build/daimon serve                   # start server on port 8090
```

## Testing

```bash
cyrius test tests/daimon.tcyr       # 225 assertions / 26 groups
cyrius bench tests/daimon.bcyr      # 17 benchmarks
sh tests/test.sh                    # tests + fuzz harnesses + libro integration smoke
```

## MCP audit tools

daimon hosts five built-in MCP tools over a hash-linked [libro](https://github.com/MacCracken/libro) audit chain — `libro_query`, `libro_verify`, `libro_export`, `libro_proof`, `libro_retention` — fed by daimon's own lifecycle and security events (agent spawn/stop, IPC auth denials, rate-limit and SSRF-guard rejections, external MCP calls). List them at `GET /v1/mcp/tools`; invoke via `POST /v1/mcp/call`.

## Documentation

- [Quickstart](docs/guides/quickstart.md) · [API reference](docs/guides/api.md) · [Architecture](docs/architecture/overview.md) · [Roadmap](docs/development/roadmap.md)
- [CHANGELOG](CHANGELOG.md) · [Security policy](SECURITY.md) · [Contributing](CONTRIBUTING.md)

## Status

Ported from Rust (9,724 LOC → 4,141 LOC Cyrius). 181 KB binary.

## License

GPL-3.0-only
