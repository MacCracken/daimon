# Daimon

**Daimon** (Greek: δαίμων — guiding spirit) — AGNOS agent orchestrator.

The core runtime for the AGNOS ecosystem: agent lifecycle, HTTP API (port 8090), process supervision, IPC, task scheduling, multi-node federation, edge fleet management, memory/vector/RAG stores, MCP tool dispatch, and screen capture.

## Building

```bash
cyrius build src/main.cyr build/daimon
```

## Status

Ported from Rust (9,724 LOC preserved in `rust-old/`). Cyrius implementation in progress.

## License

GPL-3.0-only
