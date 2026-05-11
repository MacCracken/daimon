---
name: Sandhi server max_conns accepted-but-not-honored
description: Upstream-sandhi blocker — sandhi_server_options_max_conns has the public hook but no enforcement; blocks serve_async collapse into sandhi_server_run_opts
type: blocker
status: open
filed: 2026-05-10
component: cyrius stdlib bundled sandhi (lib/sandhi.cyr, sandhi 1.3.3)
affects: daimon serve_async collapse into a single sandhi server path
---

# Sandhi server `max_conns` not enforced — `serve_async` collapse blocked

**Filed:** 2026-05-10 (during daimon 1.2.2 ship — cyrius 5.10.34, bundled sandhi 1.3.3).
**Status:** open upstream; daimon keeps its own async accept loop for now.
**Severity:** low — only blocks a refactor / dedup; security posture is unchanged (1.2.2 closed the async slowloris gap independently).

## Symptom

`lib/sandhi.cyr:11600` (bundled sandhi 1.3.3 at cyrius 5.10.34) carries:

```
# Opts-aware variant. Applies SO_RCVTIMEO on each accepted connection
# to bound slow/idle peers (slowloris guard). max_conns is accepted
# but not honored today — server stays single-threaded until 0.8.0.
```

The `max_conns` field in the `SandhiServerOptions` struct is set via
`sandhi_server_options_max_conns(opts, n)` (public, line 11572-11574)
and read via `sandhi_server_options_get_max_conns(opts)` (line 11582-11584),
but the `sandhi_server_run_opts` loop (line 11621-11648) **does not**
gate `sock_accept` on a connection-count check, never spawns a worker,
and never returns to the accept loop without finishing a request. It
remains single-flight regardless of the configured value.

## Why daimon cares

The 1.1.5 roadmap (now 1.2.x) planned a collapse:

> **Collapse `serve_async` to `sandhi_server_run_opts`** once a Cyrius
> stdlib patch enforces `sandhi_server_options_max_conns`. The hook is
> already public — only the enforcement path is reserved. When wired,
> daimon's `serve_async` (epoll loop + per-call buf alloc + inline
> smuggling-check duplication) collapses into one `sandhi_server_run_opts(...)`
> call shared with sync.

The collapse would eliminate:
- daimon's own epoll-cooperative accept loop (40 lines in `serve_async`,
  `src/main.cyr:4040-4080`)
- daimon's per-call `alloc(MAX_REQUEST_SIZE + 1)` (sandhi's
  `_hsv_req_buf` could be used safely under a multi-conn server because
  sandhi would own the no-interleave invariant)
- daimon's inline duplication of smuggling-rejection checks in
  `async_handle_client`

Net code drop: ~60 LOC, plus elimination of two parallel code paths
that have to be kept in sync.

## What daimon ships at 1.2.2 instead

1.2.2 ships the **idle_ms** half of the 1.1.5 plan (sync `serve` now
threads `sandhi_server_options_idle_ms(opts, 5000)` through
`sandhi_server_run_opts`) and **also** closes the async slowloris gap
independently — `serve_async` applies `SO_RCVTIMEO` per accepted cfd via
a new daimon-side helper `set_recv_timeout_ms` mirroring what
`sandhi_server_run_opts` does internally for sync. Async no longer has
unbounded `async_await_readable` exposure to slow senders.

Net effect: the security-relevant half of the 1.1.5 plan shipped at
1.2.2; the architectural-cleanup half (collapse) stays gated on upstream.

## What "enforced" means

Either of the following counts as upstream wiring this:

1. **In-process worker pool / fiber pool** — `sandhi_server_run_opts`
   spawns a per-request handler in a worker so the accept loop can
   take the next connection while a slow one is in-flight. Bounded
   by `max_conns`.
2. **Epoll-cooperative variant** — `sandhi_server_run_opts` integrates
   with `lib/async.cyr` directly (parallel to daimon's current
   approach), reading `max_conns` as the concurrency cap.

Either approach makes daimon's `serve_async` collapse safe — daimon
hands accept to sandhi, hands the smuggling-check stack to sandhi, and
keeps only its own `handle_request` as the per-request body.

## Daimon-side workaround

None needed today. The two-path posture (`serve` sync via sandhi opts +
`serve_async` async via daimon-owned epoll) is correct under the current
upstream state. The 1.2.2 SO_RCVTIMEO addition to async closes the
slowloris exposure without requiring the collapse.

This file tracks the collapse so it surfaces in the next cyrius pin
bump: at every bundled-sandhi version bump, re-check
`sandhi_server_run_opts` for max_conns enforcement (search for new
`worker` / `pool` / `spawn` / `epoll` calls inside the accept loop). If
present, schedule the collapse as a 1.2.x slot (small bites: collapse
serve_async first, then drop the duplicated smuggling helpers, then
remove the daimon epoll-shim usage if unused elsewhere).

## Tracking

- This file is the canonical daimon-side tracker.
- Upstream fix lives in cyrius stdlib's `lib/sandhi.cyr` (`sandhi`
  source repo, now folded into cyrius). Not filed there — passive
  tracking until a consumer-driven reason to escalate. Same passive
  posture as `2026-05-10-cyrius-async-aarch64.md`.

## Related

- 1.1.4 sandhi-migration audit, `docs/audit/2026-04-27-sandhi-migration.md` §
  Deferred → 1.1.5 plan (now this file plus 1.2.2 CHANGELOG).
- 1.2.2 CHANGELOG entry — records the idle_ms wiring + async slowloris
  close; flags the collapse as still-deferred.
