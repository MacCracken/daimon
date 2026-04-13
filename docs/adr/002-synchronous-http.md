# ADR-002: HTTP Server — Synchronous with Async Option

**Status**: Amended (2026-04-13)
**Date**: 2026-04-13
**Amendment**: Original assumed Cyrius had no async runtime. `lib/async.cyr` provides epoll-based cooperative async. Both sync and async modes should be supported.

## Decision

Ship v1.0 with the synchronous HTTP server. Add async HTTP as a near-term roadmap item using `lib/async.cyr`.

## Current: Synchronous Mode

Single-threaded TCP accept loop. One request at a time. `Connection: close` on every response.

**Advantages**:
- Simple, correct — no race conditions on shared state
- No async runtime overhead for low-traffic control plane
- 181 KB binary

**Limitations**:
- A slow handler blocks all clients
- No concurrent request handling
- No WebSocket push

## Planned: Async Mode

Use `lib/async.cyr` epoll-based cooperative runtime to handle multiple connections concurrently:

```cyrius
fn handle_client(cfd) {
    async_await_readable(cfd);
    var buf = alloc(MAX_REQUEST_SIZE);
    var n = async_read(cfd, buf, MAX_REQUEST_SIZE);
    # ... parse + route + respond ...
    sock_close(cfd);
}

fn serve_async(port) {
    var sfd = bind_and_listen(port);
    var rt = async_new();
    while (1 == 1) {
        var cfd = accept(sfd);
        async_spawn(rt, &handle_client, cfd);
        if (pending_tasks > threshold) { async_run(rt); }
    }
}
```

Available stdlib functions:
- `async_new()` — create epoll-backed runtime
- `async_spawn(rt, &fn, arg)` — schedule a task
- `async_run(rt)` — run all tasks to completion
- `async_sleep_ms(ms)` — timerfd-based sleep
- `async_await_readable(fd)` — epoll wait for fd readability
- `async_read(fd, buf, len)` — non-blocking read (O_NONBLOCK + fcntl)
- `async_timeout(fp, arg, ms)` — fork-based timeout

## Trade-offs

The cooperative runtime runs each spawned task to completion before checking the next. True concurrency within a single accept iteration requires either:
1. Tasks that yield (call `async_await_readable` to return control), or
2. Batched accept: collect N clients, spawn N tasks, `async_run` the batch

This is sufficient for the orchestrator workload pattern (many short requests) but not for long-lived WebSocket connections.

## Consequences

- CLI flag or config option: `serve --async` vs default sync
- Shared state (globals) needs no locking — cooperative tasks don't preempt
- Both modes share the same route handlers and JSON escaping
