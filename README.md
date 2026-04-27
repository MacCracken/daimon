# Daimon

**Daimon** (Greek: δαίμων — guiding spirit) — AGNOS agent orchestrator.

The core runtime for the AGNOS ecosystem: agent lifecycle, HTTP API (port 8090), process supervision, IPC, task scheduling, multi-node federation, edge fleet management, memory/vector/RAG stores, MCP tool dispatch, and screen capture.

## Building

Requires [Cyrius](https://github.com/MacCracken/cyrius) 5.7.12+ (pinned in `cyrius.cyml`).

```bash
cyrius deps           # resolve dependencies (writes cyrius.lock)
cyrius build          # build from cyrius.cyml
./build/daimon serve  # start server on port 8090
```

## Testing

```bash
cyrius test tests/daimon.tcyr       # 200 assertions / 26 groups
cyrius bench tests/daimon.bcyr      # 16 benchmarks
sh tests/test.sh                    # tests + fuzz harnesses
```

## Status

Ported from Rust (9,724 LOC → 4,141 LOC Cyrius). 181 KB binary.

## License

GPL-3.0-only
