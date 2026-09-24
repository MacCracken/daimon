---
name: Daimon Documentation Health
description: Living state of doc currency in the daimon repo — fresh / stale / archived / open-question, refreshed as docs are touched
type: state
---

# Documentation Health — daimon

> **Last refresh**: 2026-09-23 (2.4.2 — 2.3.x's last items: ADR-008, new (a cgroup per agent); ADR-006's addendum on full slots; the lifecycle audit's 2.4.2 addendum; the API guide, overview, README, BENCHMARKS, SECURITY, CONTRIBUTING, CLAUDE.md, quickstart and roadmap updated.)
> **Refresh cadence**: when docs are touched, update the affected row. Full re-audit at each minor cut.
> **Scope**: this repo only (`daimon`) — root-level files plus the entire `docs/` tree.

This is a **ledger**, not a one-time audit. Rewrite-in-place as docs change. Pattern lifted from [agnosys/docs/doc-health.md](https://github.com/MacCracken/agnosys/blob/main/docs/doc-health.md) and [cyrius/docs/doc-health.md](https://github.com/MacCracken/cyrius/blob/main/docs/doc-health.md) — same buckets, daimon-shaped tiers.

Daimon is the AGNOS agent orchestrator — every consumer (hoosh, agnoshi, aethersafha, the agent fleet) depends on the HTTP API surface and the supervisor / scheduler / federation primitives. Stale endpoint docs propagate downstream, so doc currency carries weight even though the doc surface is modest today (~15 files).

---

## At a glance — 2026-07-03 inventory

**~15 markdown files** total (7 root + 8 under `docs/`). Bucket counts after the 1.3.1 doc sweep:

| Bucket | Count | What it means |
|---|---|---|
| ✅ **Fresh — refreshed in the 1.3.0 / 1.3.1 cycle** | ~13 | CHANGELOG (1.3.1 cut), VERSION (1.3.1), CLAUDE.md (6.3.43 / sandhi 1.7.0 / sakshi 2.4.3 / sigil / bote-libro rows), README (expanded), CONTRIBUTING, architecture/overview.md, guides/quickstart.md, guides/api.md (verified current), BENCHMARKS.md (re-baselined under 6.3.43), roadmap (trimmed to open-work-only), SECURITY.md (supported-versions rolled), this file. |
| 🟡 **Stale — refresh in place** | 0 | Cleared — the v1.2.x doc-refresh backlog is drained and the 1.3.1 sweep caught the roadmap / README / SECURITY drift. |
| 🔵 **Probably evergreen** | 2 | `CODE_OF_CONDUCT.md`, `LICENSE`. No version-tied claims. Re-read pass annually. |
| 📦 **Archive / frozen by design** | ~4 | The 3 ADRs (point-in-time decisions); audit/2026-04-13 + audit/2026-04-27 reports (frozen by audit convention). |
| ❓ **Open strategic question** | 0 | None outstanding. See [Open questions](#open-strategic-questions) below for what would re-open it. |

**Doc work shipped in 1.2.0:**
- ✅ `CHANGELOG.md` — 1.2.0 entry recording the cyrius 5.10.34 / sakshi 2.2.3 bump + CI/release rewrite + `/lib/` gitignored.
- ✅ `CLAUDE.md` — cyrius pin reference refreshed 5.7.12 → 5.10.34; sakshi line refreshed 2.0.0 → 2.2.3; sandhi note remains "in use".
- ✅ `docs/development/roadmap.md` — 1.1.5 items rescoped to 1.2.1 / 1.2.2; "Future (v1.2.0+)" renamed to "Future (v1.3.0+)".
- ✅ `docs/doc-health.md` — this file (initial scaffold; agnosys convention).

**Doc work shipped in 1.2.1:**
- ✅ `CHANGELOG.md` — 1.2.1 entry for external MCP forwarding (sandhi_rpc_mcp_call dispatch, validate_callback_url enforced at register boundary, +13 test assertions, +1 360 bytes binary).
- ✅ `docs/development/roadmap.md` — 1.2.1 marked complete; rescoping note on the original `McpToolDescription.endpoint_url` plan (rescoped to use the existing external-wrapper struct + `mcp_find_external_url` accessor).
- ✅ `docs/doc-health.md` — last-refresh date rolled.

**Doc work shipped in 1.2.2:**
- ✅ `CHANGELOG.md` — 1.2.2 entry: sync `serve` threads sandhi opts with `idle_ms = 5000`; `serve_async` applies SO_RCVTIMEO per accepted cfd (closes VULN-async-slowloris); `serve_async` collapse stays deferred (max_conns upstream).
- ✅ `docs/development/roadmap.md` — idle_ms half marked shipped; collapse half kept open with upstream pointer.
- ✅ `docs/doc-health.md` — last-refresh date rolled.

**Post-1.2.2 housekeeping (2026-05-10):**
- ✅ Daimon-side blocker trackers migrated upstream per the "blockers live where they're fixed" rule. Two files removed from `daimon/docs/development/issues/`; replaced with upstream pointers in roadmap + CHANGELOG (severity tagged in both).
- ✅ Daimon roadmap current-arc items now carry explicit severity markers.

**Doc work shipped in 1.3.0 (2026-07-03):**
- ✅ `CHANGELOG.md` — 1.3.0 entry: cyrius 6.2.11 → 6.3.43, sandhi 1.6.2 → 1.7.0, sakshi 2.3.0 → 2.4.3, sigil declared in `[deps].stdlib`; VERSION single-source-of-truth; doc refresh.
- ✅ `CLAUDE.md` — Cyrius pin 6.2.11 → 6.3.43; stdlib table refreshed (sandhi 1.7.0, sigil added, sakshi 2.4.3 row).
- ✅ `README.md`, `CONTRIBUTING.md`, `docs/architecture/overview.md`, `docs/guides/quickstart.md` — the v1.2.x stale-doc backlog drained: cyrius pin, `cyrius lib sync` + `cyrius deps` workflow, `lib/` gitignored, `[deps].stdlib` list (incl. sigil), 225-test count, sync+async HTTP correction.
- ✅ `BENCHMARKS.md` — re-baselined under cyrius 6.3.43 (current-baseline table added; frozen v1.0.1 port comparison preserved).
- ✅ `docs/guides/api.md` — verified current (24-endpoint reference carries no version-tied claims; no change needed).
- ✅ `VERSION` — 1.2.9 → 1.3.0.

**Doc work shipped in 1.3.1 (2026-07-03):**
- ✅ `docs/development/roadmap.md` — **trimmed to open-work-only**: all completed (`[x]`) sections removed (they live in CHANGELOG), the met v1.0-criteria block dropped, and the overlapping "Blocked on Upstream Ports" / current-arc sections consolidated. Now holds just the VULN-007 security gate, the nein firewall-MCP blocker, and the v1.4.0+ backlog, under a lean status header.
- ✅ `README.md` — expanded: intro notes the libro audit trail; deps example lists sakshi/bote/libro/majra; benchmark count 16 → 17; added an "MCP audit tools" section and a "Documentation" link block. (Footprint line left as-is.)
- ✅ `SECURITY.md` — supported-versions table rolled `1.0.x` → `1.3.x` (+ `< 1.3` unsupported).
- ✅ `CHANGELOG.md` / `VERSION` — 1.3.1 cut (`## [1.3.1] - Unreleased` opened; VERSION 1.3.0 → 1.3.1).

**Doc work shipped in 2.2.3 (2026-09-22):**
- ✅ `README.md`, `docs/guides/quickstart.md` — test commands and counts were 1.3-era (`225 assertions`,
  one suite, 17 benchmarks); now `cyrius tests` / `cyrius fuzz` / `tests/smoke.sh`, 645 assertions in
  16 suites, 21 benchmarks, 6 fuzz harnesses.
- ✅ `CONTRIBUTING.md` — "add tests in `tests/daimon.tcyr`" steered new work back into the copied-code
  suite the 2.2.x arc removed; now: a per-module suite that includes the real `src/` file, and fuzz
  harnesses on `fuzz/rng.cyr` with portable exits.
- ✅ `BENCHMARKS.md` — current baseline replaced with numbers from the REAL code (the old one timed
  local copies); the frozen port-era comparison kept but corrected (its two "Cyrius wins" were
  measured on copies); the correctness table's "Complete" rows for the memory store and IPC — never
  true, both crashed on first call — and "firewall: Blocked" (integrated since 2.1.8) corrected.
- ✅ `docs/development/roadmap.md` — 2.2.x closed and removed; the gaps 2.2.3 found recorded under
  2.3.x (tasks never start; IPC items), 2.5.x (memory API, registration ownership) and a P3.
- ✅ `SECURITY.md` — supported versions rolled `1.3.x` → `2.2.x` (+ `< 2.2` unsupported); it had
  named 1.3.x as the supported line through the whole 2.x series.
- ✅ `CHANGELOG.md` / `VERSION` / `src/config.cyr` — 2.2.3 cut (`scripts/version-bump.sh`).

**Doc work shipped in 2.3.0 (2026-09-22):**
- ✅ `docs/audit/2026-09-22-agent-lifecycle-audit.md` — new. VULN-011 (bound every interface),
  VULN-012 (cross-site requests / DNS rebinding), VULN-013 (descriptors leaked into agents),
  VULN-014 (a stop holds the server), VULN-015 (inherited environment). Each has observed evidence and
  CVE / CWE references.
- ✅ `docs/adr/004-agent-process-control.md` — new. The four decisions behind process control on an
  unauthenticated API: the executable is chosen by type and never by a request, loopback bind,
  browser-originated agent control refused, and a synchronous bounded stop. Rejected alternatives
  are recorded with their reasons.
- ✅ `docs/guides/api.md` — the lifecycle routes, `type` / `exit_code`, what runs and how the child
  is set up, the Origin rule, the bind address, and 403 / 405 / 409 / 500 / 501 in the error table.
- ✅ `docs/guides/quickstart.md` — a start / stop walk-through, the bind address, and counts
  (741 assertions, 22 + 2 benchmarks).
- ✅ `docs/architecture/overview.md` — agent.cyr and server.cyr entries, and CLI flags. The route
  count read "24 endpoints", which was already stale; it is now 38, counted from `src/router.cyr`.
- ✅ `README.md` — bind address, counts, and what the smoke script covers.
- ✅ `BENCHMARKS.md` — a 2.3.0 section: 3 lifecycle benchmarks and a back-to-back 2.2.3 / 2.3.0 A/B
  of the 19 existing ones. The correctness and coverage rows are updated.
- ✅ `SECURITY.md` — supported versions `2.2.x` → `2.3.x`.
- ✅ `docs/development/roadmap.md` — 2.3.x narrowed to task start/complete then IPC, with the
  lifecycle follow-ups. 2.4.x is unblocked; 2.5.x says what 2.2.2 and 2.3.0 closed and what is still
  open. Its two function names were wrong (`ipc_send`, `complete_task`); they are now
  `agent_ipc_send` and samay's `task_scheduler_complete_task`.
- ✅ `CHANGELOG.md` / `VERSION` / `src/config.cyr` — 2.3.0 (`scripts/version-bump.sh`).

**Doc work shipped in 2.4.2 (2026-09-23):**
- ✅ `docs/adr/008-agent-containment-cgroup.md` — **new**: a cgroup per agent where daimon's cgroup
  is its own. The agent moves itself in before exec, stops reach the whole cgroup, and what an agent
  left ends with it. It records the measured cost (0.25 ms a start) and the limit: not a boundary
  against an agent acting as daimon's user.
- ✅ `docs/adr/006-own-event-loop.md` — "Addendum — 2.4.2: when every slot is taken" (the oldest
  request still arriving gives way, `http_evicted`).
- ✅ `docs/audit/2026-09-22-agent-lifecycle-audit.md` — "2.4.2 addendum — the three residuals": what
  closed each, the four faults review found in containment before release, and what is left.
- ✅ `docs/guides/api.md` — slot eviction; `"contained"` in the start example; a Containment
  paragraph; `http_evicted`.
- ✅ `docs/architecture/overview.md` — containment in `agent.cyr`; `_srv_slot_for_new`.
- ✅ `docs/development/roadmap.md` — 2.3.x closed; status (2.4.2, ai-hwaccel 2.3.29, 1087 tests,
  the aarch64 VM).
- ✅ `README.md`, `docs/guides/quickstart.md`, `CONTRIBUTING.md` — `tests/aarch64/run.sh`, the
  containment checks, 1087 assertions.
- ✅ `SECURITY.md` — agent containment and connection slots in scope, with their known limits.
- ✅ `BENCHMARKS.md` — the 2.4.2 section; coverage rows.
- ✅ `CLAUDE.md` — "Commands (verified at 2.4.2)": the aarch64 VM and the containment checks; the
  agent-process rule (`_agent_spawn`, `_agent_signal_all`, exact-match markers); upkeep on the tick
  must not allocate; the ai-hwaccel row (2.3.29).
- ✅ `CHANGELOG.md` / `VERSION` / `src/config.cyr` — 2.4.2 (`scripts/version-bump.sh`).
- ✅ After CI's AGNOS guest test failed on 2.4.2: `docs/audit/2026-09-23-agnos-platform-audit.md` AG-15
  (and AG-13's 30 s corrected), the CHANGELOG's Fixed entry, CLAUDE.md's agnos conventions (the
  read bound, the console's cost), the roadmap's cyrius filings, ADR-007's 2.4.2 addendum, the API
  guide's note on forwarded calls; a second cyrius filing recorded under the upstream trackers.

**Doc work shipped in 2.4.1 (2026-09-23):**
- ✅ `docs/adr/007-daimon-on-agnos.md` — a 2.4.1 addendum: capacity (`max_agents` stays), the clock
  (`daimon_now_ms`), the guest test in CI, answers written 1 KB at a time, and detached calls on
  agnos. It corrects decision 2's "agnos has no fork for daimon's use", which was never checked and
  is wrong.
- ✅ `docs/audit/2026-09-23-agnos-platform-audit.md` — AG-11 to AG-14, "What 2.4.1 fixed", and the
  verification. AG-5 now names the detached calls' pipes.
- ✅ `docs/guides/api.md` — "On AGNOS": room (503), and answers written 1 KB at a time; the 503 row;
  detached calls on AGNOS.
- ✅ `docs/guides/agent-ipc.md` — the machine's 16 channels and the 503.
- ✅ `docs/architecture/overview.md` — `server_detach` on agnos; the clock and the paced writes.
- ✅ `docs/development/roadmap.md` — status (2.4.1, ai-hwaccel 2.3.27, the guest test in CI); twelve
  agnos filings and the cyrius one; daimon's own agnos items closed.
- ✅ `README.md`, `docs/guides/quickstart.md`, `CONTRIBUTING.md` — `tests/agnos/run.sh --release`;
  1050 assertions, 92 guest checks; `daimon_now_ms` and `daimon_write_all`.
- ✅ `SECURITY.md` — AGNOS scope: detached calls, and large answers to a local client.
- ✅ `BENCHMARKS.md` — the 2.4.1 section; coverage rows.
- ✅ `CLAUDE.md` — "Commands (verified at 2.4.1)": `run.sh --release` and the lock rebuild; the
  AGNOS conventions (`daimon_now_ms`, 1 KB writes, the kernel's room); the ai-hwaccel row (2.3.27).
- ✅ `CHANGELOG.md` / `VERSION` / `src/config.cyr` — 2.4.1 (`scripts/version-bump.sh`).

**Doc work shipped in 2.4.0 (2026-09-23):**
- ✅ `docs/adr/007-daimon-on-agnos.md` — **new**: the kernel's primitives behind daimon's seams, the
  polled loop, one wire format for channels, and the rule that an agnos gap is filed with agnos,
  with daimon's interim for each in one table.
- ✅ `docs/audit/2026-09-23-agnos-platform-audit.md` — **new**: AG-1 to AG-10. Each has its kernel
  citation, its measurement, its agnos filing, and daimon's interim.
- ✅ `docs/guides/api.md` — an "On AGNOS" section: start, limits, stop, capture, listener. The 501 row
  and the start line are corrected (no longer "501 on AGNOS until 2.4.x"). `limits_enforced` is in
  the start example.
- ✅ `docs/guides/agent-ipc.md` — an "On AGNOS" section: the fd, 64-byte records, the 4092-byte frame.
- ✅ `docs/architecture/overview.md` — the agnos arms. The data flow still drew 2.3.3's channel
  thread and hand-off; it is now the loop's.
- ✅ `docs/development/roadmap.md` — the status header; 2.4.0 shipped; what waits on each agnos
  filing and what daimon changes when it closes.
- ✅ `README.md`, `docs/guides/quickstart.md`, `CONTRIBUTING.md` — the agnos guest test; 1038.
- ✅ `SECURITY.md` — supported 2.4.x; AGNOS scope.
- ✅ `BENCHMARKS.md` — the 2.4.0 section (Linux A/B; no AGNOS performance claim); coverage rows.
- ✅ `CLAUDE.md` — "Commands (verified at 2.4.0)" with the guest test; the AGNOS conventions (never
  `sleep_ms`, the foreground slot, file gaps with agnos); the ai-hwaccel row (2.3.24, no `path`).
- ✅ `CHANGELOG.md` / `VERSION` / `src/config.cyr` — 2.4.0 (`scripts/version-bump.sh`).

**Doc work shipped in 2.3.4 (2026-09-22):**
- ✅ `docs/adr/006-own-event-loop.md` — **new**. Why daimon runs its own loop (one thread, one epoll
  set) instead of sandhi's serve loops and a channel thread; what stays sandhi's (framing, smuggling
  checks, senders); the rejected alternatives; the consequences, including the handlers that still
  block.
- ✅ `docs/adr/005-agent-channels.md` — status: §2 (the service thread) superseded by ADR-006.
- ✅ ADR-006, the API guide, the audit addendum, CLAUDE.md, overview — calls that wait on another
  server run in a child (`server_detach`); 504 after 60 s. The API guide's error table gains 502,
  which MCP forwarding has answered since 1.2.1 without a row.
- ✅ `docs/guides/agent-ipc.md` — reply 4 (`NACK_NO_TARGET`), replies given after routing, names
  (first wins), the bus byte limit, the HTTP message routes, the two new metrics.
- ✅ `docs/guides/api.md` — who may call (Host allowlist, foreign Origin), the event loop, stop
  semantics, the message and output routes, `--agent-env` / `--agent-output`, edge capabilities,
  metrics, errors.
- ✅ `docs/audit/2026-09-22-agent-lifecycle-audit.md` — the 2.3.4 addendum. VULN-012's remainder,
  VULN-014 and VULN-018 fixed with evidence; VULN-015 given an operator option; CVE-2007-6750
  (slowloris) cited for the loop's deadlines.
- ✅ `docs/architecture/overview.md` — the loop, the message layout, captured output; the route
  count (41 → 44, counted from `src/router.cyr`).
- ✅ `docs/development/roadmap.md` — 2.3.4 done and removed; what stays open is listed with the reason
  (handlers that block, `setsid` escaping the group stop, the 128-connection cap, aarch64
  `RLIMIT_AS` unverified on hardware).
- ✅ `README.md`, `docs/guides/quickstart.md` — 1033 assertions; the message and output routes; `help`.
- ✅ `BENCHMARKS.md` — the 2.3.4 section: the allocation-lock A/B, HTTP on the 200 and 429 paths, the
  poll-versus-epoll scan cost, the suite A/B. The ipc and coverage rows are updated.
- ✅ `SECURITY.md` — the IPC and browser-reach scope.
- ✅ `CONTRIBUTING.md` — the endpoint rules: Host / Origin coverage and not blocking the event loop.
  Its setup named cyrius 6.3.43 and `cyrius check` as "format + lint + test + build"; it is a syntax
  check (`cyrius help`). It now names the 6.6.6 pin and the real gate.
- ✅ `CLAUDE.md` — the event-loop conventions, the `thread` row, "Commands (verified at 2.3.4)".
- ✅ `CHANGELOG.md` / `VERSION` / `src/config.cyr` — 2.3.4 (`scripts/version-bump.sh`).

**Doc work shipped in 2.3.3 (2026-09-22):**
- ✅ `docs/guides/agent-ipc.md` — **new**: the wire protocol an agent speaks on fd 3. It covers the
  frame, the body, the replies, what closes a channel, where messages go, the metrics, sh and Python
  examples, and the limits with their constants.
- ✅ `docs/adr/005-agent-channels.md` — **new**. It records why a socketpair per agent, why a
  service thread, and the bounds. Its consequences include the allocation-lock cost, measured.
- ✅ `docs/audit/2026-09-22-agent-lifecycle-audit.md` — the 2.3.3 addendum:
  - the three parked socket-file defects, resolved by removal;
  - the new surface against D-Bus CVE-2014-3638 / -3639 and journald CVE-2018-16865;
  - the three defects found and fixed before release;
  - the thread-safety analysis;
  - VULN-018 (heap growth per message, open).
- ✅ `docs/audit/2026-04-13-security-audit.md` — VULN-006 marked superseded.
- ✅ `docs/guides/api.md` — agents talk back on fd 3; the metrics example was missing fields and is
  now the real output, with the three new counters.
- ✅ `docs/architecture/overview.md` — the ipc module map and the agent data flow. The "none over
  ~350 LOC" claim was already stale (agent.cyr ~800, api_mcp.cyr 472); it now names the largest.
- ✅ `README.md`, `docs/guides/quickstart.md` — 917 assertions, 27 + 2 benchmarks, the IPC guide
  linked. README's audit-event list named "IPC auth denials" from the removed socket code.
- ✅ `BENCHMARKS.md` — the 2.3.3 section: the broadcast A/B, the two channel benchmarks and the
  suite with the thread running. The ipc and coverage rows are updated.
- ✅ `SECURITY.md` — the IPC scope line describes the channel, not socket files.
- ✅ `CLAUDE.md` — the thread conventions (exit_group, what the service thread may touch, the fork
  child), the include rule for `src/agent.cyr`, and judging a suite by its exit status. The `thread`
  and `libro` rows are updated.
- ✅ `docs/development/roadmap.md` — 2.3.3 done. Message routes are next, then the channel
  follow-ups: the allocation lock, VULN-018 and delivery without HTTP traffic.
- ✅ `CHANGELOG.md` / `VERSION` / `src/config.cyr` — 2.3.3 (`scripts/version-bump.sh`).

**Doc work shipped in 2.3.2 (2026-09-22):**
- ✅ `docs/guides/api.md` — a **Request bodies** section: one JSON object, strings decoded, the
  refusals, and field types. The 2.3.1 known-issue note is removed, because the issue is fixed.
- ✅ `CONTRIBUTING.md` — a new **Adding an HTTP Endpoint** section: read bodies with
  `http_body_json` / `http_json_*`, never bayan's flat `json_parse`.
- ✅ `docs/audit/2026-09-22-agent-lifecycle-audit.md` — the 2.3.2 addendum: VULN-017, the
  flat-parser differential.
- ✅ `docs/architecture/overview.md` — `http.cyr`'s request-body readers.
- ✅ `README.md`, `docs/guides/quickstart.md` — 833 assertions, 24 + 2 benchmarks, 7 fuzz harnesses.
- ✅ `BENCHMARKS.md` — the 2.3.2 section (the new read against the flat baseline, and the
  2.3.1 / 2.3.2 A/B) and the coverage rows.
- ✅ `docs/development/roadmap.md` — 2.3.2 done, IPC next, and a P3 for edge ids sharing the agent
  counter.
- ✅ `CHANGELOG.md` / `VERSION` / `src/config.cyr` — 2.3.2 (`scripts/version-bump.sh`).

**Doc work shipped in 2.3.1 (2026-09-22):**
- ✅ `docs/guides/api.md` — the task start / complete routes, a node's work list, the new task JSON
  and stats fields, and a task's life through the API. Also a **known-issue** note: request strings
  keep their JSON escapes until 2.3.2.
- ✅ `docs/audit/2026-09-22-agent-lifecycle-audit.md` — addendum: VULN-016, any client can report
  any task's state.
- ✅ `docs/architecture/overview.md` — `sched.cyr`, and the route count 38 → 41 (counted from
  `src/router.cyr`).
- ✅ `README.md`, `docs/guides/quickstart.md` — counts: 797 assertions, 17 suites, 23 + 2 benchmarks.
  Both still called the binary **181 KB**, the v1.0.1 port-era figure; it is 3,223,048 bytes, of
  which 1,613,123 are unreachable code the 6.x toolchain NOPs in place (build output, 2.3.1).
- ✅ `BENCHMARKS.md` — the 2.3.1 benchmark and the 2.3.0 / 2.3.1 A/B; the scheduler, api and
  coverage rows.
- ✅ `docs/development/roadmap.md` — 2.3.1 done. 2.3.2 (request-string decoding) is next, then IPC,
  and VULN-016 sits under identity.
- ✅ `CHANGELOG.md` / `VERSION` / `src/config.cyr` — 2.3.1 (`scripts/version-bump.sh`).

⚠ This ledger said "stale set: cleared" through the whole 2.x line while README quoted 1.3-era test
counts and BENCHMARKS.md called untested modules "Complete". A doc can only be as fresh as the check
that reads it: the rows below are what was last *recorded*, not a guarantee.

**Stale set:** none recorded. `CLAUDE.md` was refreshed on 2026-09-22 at the maintainer's request.
Each change was checked against `cyrius.cyml`, the source or the tool's own help:
- pins: cyrius 6.6.6, samay 1.1.3, bote 3.3.13, libro 2.10.3, majra 2.9.1, with bayan 1.5.6 and
  nein 1.7.0 added;
- the `bayan` row: request bodies are read with `http_body_json` / `http_json_*`, and
  `json_escape_str` lives in `src/error.cyr`;
- sandhi's serve entry points, and the `http_*` shims in `src/http.cyr`;
- "CalVer" became `MAJOR.MINOR.PATCH`;
- `cyrius check` (a syntax check only) is replaced by the real gate under **Commands**;
- the Rust tags are `0.5.0` / `0.6.0`;
- new sections: **Commands**, **Conventions that bite**, the version-naming rule, and two DO NOTs.

---

## Tier 1 — Root files

| File | Last touched | Status | Notes |
|---|---|---|---|
| `README.md` | 2026-09-23 | ✅ Fresh | 2.4.2: the aarch64 VM, containment and connection slots in the smoke line. 2.4.1: `run.sh --release` (CI), 92 guest checks, detached calls on AGNOS. 2.4.0: 1038 assertions; the agnos guest test; an "On AGNOS" paragraph. |
| `CHANGELOG.md` | 2026-09-23 | ✅ Fresh | Source of truth for shipped work. 2.4.2 entry: containment, slot eviction, the aarch64 VM, ai-hwaccel 2.3.29. 2.4.1 entry: detached calls, the clock and 1 KB writes on AGNOS, the 503, the guest test in CI, the lock verified, four filings. |
| `CLAUDE.md` | 2026-09-23 | ✅ Fresh | Durable rules. 2.4.2: `_agent_spawn` / `_agent_signal_all`, exact-match markers, no allocation on the tick, the aarch64 VM, ai-hwaccel 2.3.29. 2.4.1: `daimon_now_ms`, 1 KB writes, the kernel's room, `run.sh --release`, the lock rebuild, ai-hwaccel 2.3.27. 2.4.0: never sleep_ms, the foreground slot, file gaps with agnos. |
| `CONTRIBUTING.md` | 2026-09-23 | ✅ Fresh | 2.4.2: `tests/aarch64/run.sh`; changes to starting and stopping agents are run contained. 2.4.1: `run.sh --release`; the clock and write shims. 2.4.0: the agnos guest test and testing agnos arms; the six endpoint rules; the 6.6.6 pin and the real gate. |
| `SECURITY.md` | 2026-09-23 | ✅ Fresh | Supported versions `2.4.x`. 2.4.2: agent containment and connection slots in scope, with their known limits. 2.4.1: detached calls and large answers in the AGNOS scope. 2.4.0: AGNOS scope and the platform audit. |
| `CODE_OF_CONDUCT.md` | (initial) | 🔵 Evergreen | Standard. |
| `BENCHMARKS.md` | 2026-09-23 | ✅ Fresh | A section per release with its A/B; 2.4.2: the contained start's cost and the Linux A/B; 2.4.1 and 2.4.0: Linux A/B, no AGNOS performance claim. Frozen v1.0.1 port comparison kept. |
| `VERSION` | 2026-09-23 | ✅ Fresh | `2.4.2`, written with `src/config.cyr` by `scripts/version-bump.sh`; `tests/version_sync.tcyr` checks they agree. |
| `LICENSE` | (initial) | 🔵 Evergreen | GPL-3.0-only. |

---

## Tier 2 — Architecture (`docs/architecture/`)

| File | Last touched | Status | Notes |
|---|---|---|---|
| `overview.md` | 2026-09-23 | ✅ Fresh | 2.4.2: containment; `_srv_slot_for_new`. 2.4.1: `server_detach` on agnos, the clock, paced writes. 2.4.0: the agnos arms; the data flow no longer draws 2.3.3's channel thread. |

---

## Tier 3 — Project state (`docs/development/`)

| File | Last touched | Status | Notes |
|---|---|---|---|
| `roadmap.md` | 2026-09-23 | ✅ Fresh | Open work only. 2.4.2: 2.3.x closed; 1087 tests; the aarch64 VM. 2.4.1: daimon's own agnos items closed; twelve agnos filings and one cyrius filing, and what daimon changes when each closes; 2.5.x. |

**Missing today (file in 1.2.x cleanup):**
- `development/state.md` — agnosys convention for the live volatile state file (pin / build sizes / test count / consumer table / recent releases / slot ledger). Daimon's roadmap.md partially covers this; consider splitting in 1.2.x if scope grows.
- `development/capability-map.md` — auto-generated per-module kernel-surface map. Daimon's surface is mostly userland (HTTP API + IPC over Unix sockets), so the security value is smaller than for agnosys; flag as nice-to-have, not P1.

**Tier — Engineering issues (upstream trackers)**

⚠ **Corrected 2.1.7.** This line read *"Daimon does not carry its own `docs/development/issues/` directory"* — false since 1.2.x. Daimon carries `docs/development/issues/` for filings it owns or co-owns, with resolved ones moved to `issues/archive/`. As of **2.3.4** there are **no open filings** and **eight archived** (the 2.1.7 count named one open filing that was already in `archive/`). The table below tracks filings that live in an UPSTREAM repo's tracker.

| Tracker | Severity | Filed | Status | Notes |
|---|---|---|---|---|
| [cyrius § daimon-async-aarch64-sys-epoll-wait](https://github.com/MacCracken/cyrius/blob/main/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md) | **P2** | 2026-05-10 | ✅ **RESOLVED upstream — verified 2.1.7** | `lib/async.cyr` no longer references `SYS_EPOLL_WAIT` at all; the aarch64 peer defines `SYS_EPOLL_PWAIT = 1022` (a private alias, since native 22 is taken by x86 `pipe`). Confirmed end-to-end: daimon's `--aarch64` cross-build succeeds and the binary serves under `qemu-aarch64`. CI's warn-on-detect allowlist for this symbol now matches nothing — kept for older pins. |
| agnos `2026-09-23-inbound-tcp-syn-dropped-by-isr-drain.md` | **High** (daimon's view) | 2026-09-23 | 🟠 Open upstream | A SYN drained in interrupt context is dropped: servers accept 0–9 of 12. daimon's API is reachable only from the box. |
| agnos `2026-09-23-socket-ids-have-no-owner.md` | **High** | 2026-09-23 | 🟠 Open upstream | Any process can read, write or close any TCP connection (audit AG-1). |
| agnos `2026-09-23-parent-cannot-end-stop-or-continue-a-child.md` | Medium | 2026-09-23 | 🟠 Open upstream | No default signal actions: a stop only asks; pause/resume 501. |
| agnos `2026-09-23-no-per-process-resource-limits.md` | Medium | 2026-09-23 | 🟠 Open upstream | Agents run without limits (operator ruling), audited. |
| agnos `2026-09-23-tcp-server-cannot-be-loopback-only.md` | Medium | 2026-09-23 | 🟠 Open upstream | No listen address; 127.0.0.1 TCP dropped; no peer address. |
| agnos `2026-09-23-child-inherits-every-fd-and-spawn-arms-leak.md` | Medium | 2026-09-23 | 🟠 Open upstream | No close-on-exec; one redirect; CH_ENDOW armed after a failed spawn. Capture refused on agnos. |
| agnos `2026-09-23-sleep-ms-holds-the-cpu.md` | Low | 2026-09-23 | 🟠 Open upstream | daimon waits with `daimon_yield_ms`. |
| agnos `2026-09-23-sock-recv-never-reports-eof-after-peer-fin.md` | Low | 2026-09-23 | 🟠 Open upstream | CLOSE_WAIT reads as "nothing yet". |
| agnos `2026-09-23-spawn-path-args-cannot-contain-spaces.md` | Low | 2026-09-23 | 🟠 Open upstream | 422 for a name with a space. |
| agnos `2026-09-23-sock-send-and-connect-hold-the-cpu.md` (2.4.1) | **High** (daimon's view) | 2026-09-23 | 🟠 Open upstream | A send over 2 KB to a local process stops the machine; a connect holds the CPU up to ~8 s. daimon writes 1 KB at a time (AG-14). |
| agnos `2026-09-23-tsc-calibration-refused-stops-the-us-clock.md` (2.4.1) | Medium | 2026-09-23 | 🟠 Open upstream | One calibration at boot, refused or wrong under a CPU quota. daimon falls back to the tick (AG-12). |
| agnos `2026-09-23-spawn-path-failure-gives-no-reason.md` (2.4.1) | Low | 2026-09-23 | 🟠 Open upstream | A full process table and a bad executable both answer -1 (AG-11). |
| cyrius `2026-09-23-daimon-agnos-clock-stands-still-when-tsc-calibration-refused.md` (2.4.1) | Medium | 2026-09-23 | 🟠 Open upstream | `clock_now_ns` on agnos does not check `#95`'s -1. daimon uses `daimon_now_ms`. |
| cyrius `2026-09-23-daimon-agnos-socket-read-gives-up-after-a-second.md` (2.4.2) | Medium-high | 2026-09-23 | 🟠 Open upstream | `_agnos_sock_recv_block`'s 6000-pause bound ran out in ~1 s: a forwarded call to a server slower than that was answered 502, and CI's guest test failed on it. daimon lifts the bound (`daimon_agnos_recv_bound`). |
| [sandhi § daimon-server-max-conns](https://github.com/MacCracken/sandhi/blob/main/docs/development/issues/archive/2026-05-10-daimon-server-max-conns.md) | **Low** | 2026-05-10 | ✅ **RESOLVED both sides — corrected 2.1.7** | ⛔ This row read "Open upstream / no daimon-side action", which was wrong on both halves. Sandhi shipped the epoll-cooperative `max_conns` enforcement in **`sandhi_server_run_async` at 1.4.9** (hardened 1.4.10) and ARCHIVED its filing; daimon collapsed `serve_async` onto that call at **1.2.6** (`src/server.cyr:224-244`). Nothing is open on either side. The `"reserved for 0.8.0+"` text still in `lib/sandhi.cyr` is a stale doc comment on the **sync** options struct (`sandhi_server_run_opts`), not the async path daimon uses. |

---

## Tier 4 — ADRs (`docs/adr/`)

| File | Last touched | Status | Notes |
|---|---|---|---|
| `001-rust-to-cyrius-port.md` | 2026-04-13 | 📦 Frozen | Accepted (0.7.0). Rust → Cyrius port rationale. Historical record. |
| `002-synchronous-http.md` | 2026-04-13 | 📦 Frozen | Accepted, then partially superseded by 1.1.0 (async via lib/async.cyr) and again by 1.1.4 (sandhi adoption). The ADR's "invalid" note is captured in CLAUDE.md; re-read at v2.0 to decide whether to revise or supersede with a new ADR. |
| `003-security-audit-process.md` | 2026-04-13 | 📦 Frozen | Accepted (0.7.0). P(-1) + Work-Loop audit cadence. Verified by every release since; the rule holds. |
| `004-agent-process-control.md` | 2026-09-22 | ✅ Accepted | 2.3.0. Process control on an unauthenticated API: executable by type, loopback bind, browser control refused, bounded stop. |
| `005-agent-channels.md` | 2026-09-22 | ✅ Accepted, §2 superseded | 2.3.3. A socketpair per agent on fd 3; §2 (the service thread) superseded by ADR-006 at 2.3.4. |
| `006-own-event-loop.md` | 2026-09-23 | ✅ Accepted | 2.3.4. daimon's own epoll loop over sandhi's HTTP; the serving model since 2.3.4. 2.4.2 addendum: when every slot is taken. |
| `007-daimon-on-agnos.md` | 2026-09-23 | ✅ Accepted | 2.4.0. The agnos kernel's primitives behind daimon's seams; the polled loop; gaps filed with agnos. 2.4.1 addendum: capacity, the clock, CI, 1 KB writes, detached calls, and the corrected fork claim. 2.4.2 addendum: a detached call's read waits for the RTC (`daimon_agnos_recv_bound`). |
| `008-agent-containment-cgroup.md` | 2026-09-23 | ✅ Accepted | 2.4.2. A cgroup per agent where daimon's cgroup is its own; uncontained, warned and audited elsewhere. |

**ADR posture**: low decision-velocity. Only architecturally significant calls earn an ADR — minor decisions ride CHANGELOG + design comments. 1.1.4 sandhi adoption was a candidate but rode the CHANGELOG entry; the migration audit at `docs/audit/2026-04-27-sandhi-migration.md` carries the deep rationale. Re-evaluate at v2.0.0 cut.

---

## Tier 5 — Audit reports (`docs/audit/`)

Date-stamped, frozen by design. Each P(-1) hardening pass per CLAUDE.md cadence lands a new report — old reports stay verbatim as the historical record.

| File | Date | Status | Notes |
|---|---|---|---|
| `2026-04-13-security-audit.md` | 2026-04-13 | 📦 Frozen | 0.7.0 P(-1) security audit. 10 findings, 9 fixed at 0.7.0, VULN-007 gated. |
| `2026-04-27-sandhi-migration.md` | 2026-04-27 | 📦 Frozen | 1.1.4 sandhi adoption — VULN-001 strengthened, VULN-008 trade-off documented (sandhi's 30s SO_RCVTIMEO replaces no-timeout), 1.1.5 sandhi follow-ups (now 1.2.1 / 1.2.2). |
| `2026-09-22-agent-lifecycle-audit.md` | 2026-09-23 | ✅ Open (addenda per 2.3.x release) | VULN-011 – VULN-018. 2.4.2 addendum: the three residuals closed, four containment faults found in review and fixed, what is left. 2.3.4 addendum: VULN-012 remainder, VULN-014 and VULN-018 fixed; VULN-015 an operator option; VULN-016 waits for 2.5.x identity. |
| `2026-09-23-agnos-platform-audit.md` | 2026-09-23 | ✅ Open | AG-1 – AG-15: the agnos kernel's (and, AG-12 and AG-15, cyrius's agnos code's) gaps as they reach daimon, each filed upstream, with daimon's interim. 2.4.2 fixed AG-15 in daimon. 2.4.1 fixed AG-12 and AG-13 in daimon, and mitigates AG-14. |

Next audit slot: the CLAUDE.md cadence sets the trigger (security-touching work, or a CVE pattern in daimon's surfaces: its event loop and sandhi's HTTP framing, bayan's JSON parser, the agent channels, /proc scrape paths, the bump allocator).

---

## Tier 6 — Guides (`docs/guides/`)

| File | Last touched | Status | Notes |
|---|---|---|---|
| `api.md` | 2026-09-23 | ✅ Fresh | 2.4.2: slot eviction, `contained`, Containment, `http_evicted`. 2.4.1: room and the 503, 1 KB answers, detached calls on AGNOS. 2.4.0: "On AGNOS" (start, limits, stop, capture, listener); limits_enforced. 2.3.4: who may call, the loop, stops, messages, output. |
| `quickstart.md` | 2026-09-23 | ✅ Fresh | 2.4.2: 1087 assertions. 2.4.1: 1050 assertions; `run.sh --release`. 2.4.0: tests/agnos. |
| `agent-ipc.md` | 2026-09-23 | ✅ Fresh | 2.3.3, new: the fd-3 wire protocol. 2.3.4: reply 4, names, the HTTP message routes. 2.4.0: the AGNOS channel. 2.4.1: the machine's 16 channels, the 503. |

---

## Open strategic questions

None outstanding. The following are tracked elsewhere (issue tickets / roadmap), not strategic questions:

- **External MCP forwarding** via `sandhi_rpc_mcp_call` — sequenced into 1.2.1 per the working-loop discipline. Replaces the `api_mcp_call` "tool dispatch not available in sync mode" stub.
- **Sandhi `idle_ms` tuning + `serve_async` collapse** — sequenced into 1.2.2; predicated on whether stdlib `sandhi_server_options_max_conns` enforcement landed by 6.3.43 (premise-check first).
- **jnana / gRPC / WebSocket / distributed tracing / agent migration** — deferred to v1.3.0+ per the roadmap.

Reopen the strategic-questions bucket if:
- A new consumer (hoosh, agnoshi, aethersafha) drives a transport choice we haven't made.
- The federation / scheduler primitives need a multi-host coherence story (cross-node agent migration is in v1.3.0+; if a consumer asks for it sooner, that's a strategic question).
- A CVE class hits daimon's attack surface (HTTP + Unix-socket IPC + /proc scrape + bump-allocator memory zeroing).

---

## Refresh discipline

When you touch a doc:
1. Update its row in this file (Last touched date + Status bucket if it changed).
2. If the doc shipped a substantive change (not just a typo), note the change in the relevant tier's narrative.
3. If a bucket count shifts, update the at-a-glance summary.

When a release ships:
1. Roll the "doc work shipped in X.Y.Z" block in this file's at-a-glance summary.
2. Re-audit the **Stale** bucket: anything that should have been refreshed during the release cycle but wasn't carries forward as a 1.X.(Y+1) doc cleanup pass.
3. Renumber the "Last refresh" line at the top.
