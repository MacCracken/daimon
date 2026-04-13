# ADR-002: Synchronous HTTP Server

**Status**: Accepted
**Date**: 2026-04-13
**Context**: Rust daimon used tokio + axum for async HTTP. Cyrius does not have an async runtime.

## Decision

Implement a synchronous, single-threaded TCP accept loop for the HTTP API.

## Rationale

1. **Simplicity** — No async runtime complexity, no task scheduling, no waker machinery. The server is ~100 lines of straightforward socket code.
2. **Correctness** — No race conditions on shared state. All globals are accessed sequentially. No need for RwLock/Mutex.
3. **Sufficient for workload** — Agent orchestration is control-plane traffic, not data-plane. Typical request rates are 10-100 req/s, well within single-threaded capacity.
4. **Connection: close** — Every response includes `Connection: close`. No keep-alive, no connection pooling. Eliminates HTTP/1.1 pipelining complexity and request smuggling surface area.

## Trade-offs

- **No concurrent requests** — One request at a time. A slow handler blocks all other clients.
- **No WebSocket** — Streaming events require polling, not push.
- **No HTTP/2** — Single-stream only.

## Mitigation

- Rate limiting (120 req/min per IP) prevents single-client DoS.
- Handlers are fast (sub-millisecond for most endpoints).
- Future: async HTTP when Cyrius async patterns mature (roadmap item).

## Consequences

- API latency benchmarks not directly comparable to Rust (in-process axum vs network TCP).
- Clients should use short timeouts and retry on connection refusal.
