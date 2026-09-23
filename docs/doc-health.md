---
name: Daimon Documentation Health
description: Living state of doc currency in the daimon repo — fresh / stale / archived / open-question, refreshed as docs are touched
type: state
---

# Documentation Health — daimon

> **Last refresh**: 2026-09-22 (2.3.2 — request-string decoding: API guide, CONTRIBUTING, overview, README, quickstart, BENCHMARKS, roadmap, and a second addendum to the lifecycle audit. Rows below that predate 2.3.2 are as last recorded.)
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
| `README.md` | 2026-07-03 | ✅ Fresh | Cyrius pin → 6.3.43; `cyrius lib sync` + `cyrius deps` build block; deps example lists sakshi/bote/libro/majra; 225 tests / 17 benchmarks; added MCP-audit-tools + Documentation-links sections (1.3.1). Footprint line left as-is. |
| `CHANGELOG.md` | 2026-05-10 | ✅ Fresh | Source of truth for shipped work. 1.2.0 entry covers toolchain bump, sakshi bump, CI/release modernization, /lib/ gitignored, lint-clean, fmt re-enabled. |
| `CLAUDE.md` | 2026-05-10 | ✅ Fresh | Durable rules. 1.2.0 pin refreshes: cyrius 6.3.43, sakshi 2.4.3. |
| `CONTRIBUTING.md` | 2026-07-03 | ✅ Fresh | Cyrius pin → 6.3.43; workflow updated (`cyrius lib sync` + `cyrius deps`, explicit build, `cyrius tests`); lib/ gitignored note; module-split note. |
| `SECURITY.md` | 2026-07-03 | ✅ Fresh | Supported-versions table rolled `1.0.x` → `1.3.x` (+ `< 1.3` unsupported) in the 1.3.1 sweep. Reporting policy + scope (incl. bump-allocator memory safety, VULN-007) unchanged. |
| `CODE_OF_CONDUCT.md` | (initial) | 🔵 Evergreen | Standard. |
| `BENCHMARKS.md` | 2026-07-03 | ✅ Fresh | Re-baselined under cyrius 6.3.43 (17-benchmark current-baseline table added); frozen Rust-vs-Cyrius v1.0.1 port comparison preserved; mcp / http-forward status rows updated. |
| `VERSION` | 2026-05-10 | ✅ Fresh | `1.3.0` — single source of truth, read into `cyrius.cyml` via `${file:VERSION}`. |
| `LICENSE` | (initial) | 🔵 Evergreen | GPL-3.0-only. |

---

## Tier 2 — Architecture (`docs/architecture/`)

| File | Last touched | Status | Notes |
|---|---|---|---|
| `overview.md` | 2026-07-03 | ✅ Fresh | Dependency design-decision refreshed: full `[deps].stdlib` list incl. sigil, sandhi 1.7.0 / sigil 3.10.0 / sakshi 2.4.3, lib/ gitignored, cyrius.lock 61 deps. Design-decision #2 corrected to sync+async HTTP. |

---

## Tier 3 — Project state (`docs/development/`)

| File | Last touched | Status | Notes |
|---|---|---|---|
| `roadmap.md` | 2026-07-03 | ✅ Fresh | **Trimmed to open-work-only (1.3.1)**: all completed `[x]` sections + met v1.0-criteria removed (history lives in CHANGELOG); consolidated to the VULN-007 gate, the nein firewall-MCP blocker, and the v1.4.0+ backlog. |

**Missing today (file in 1.2.x cleanup):**
- `development/state.md` — agnosys convention for the live volatile state file (pin / build sizes / test count / consumer table / recent releases / slot ledger). Daimon's roadmap.md partially covers this; consider splitting in 1.2.x if scope grows.
- `development/capability-map.md` — auto-generated per-module kernel-surface map. Daimon's surface is mostly userland (HTTP API + IPC over Unix sockets), so the security value is smaller than for agnosys; flag as nice-to-have, not P1.

**Tier — Engineering issues (upstream trackers)**

⚠ **Corrected 2.1.7.** This line read *"Daimon does not carry its own `docs/development/issues/` directory"* — false since 1.2.x. Daimon carries `docs/development/issues/` for filings it owns or co-owns, with resolved ones moved to `issues/archive/`. As of **2.1.7** there is exactly **one open filing** ([`2026-09-14-daimon-does-not-build-for-agnos.md`](development/issues/archive/2026-09-14-daimon-does-not-build-for-agnos.md)) and **seven archived**. The table below tracks filings that live in an UPSTREAM repo's tracker.

| Tracker | Severity | Filed | Status | Notes |
|---|---|---|---|---|
| [cyrius § daimon-async-aarch64-sys-epoll-wait](https://github.com/MacCracken/cyrius/blob/main/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md) | **P2** | 2026-05-10 | ✅ **RESOLVED upstream — verified 2.1.7** | `lib/async.cyr` no longer references `SYS_EPOLL_WAIT` at all; the aarch64 peer defines `SYS_EPOLL_PWAIT = 1022` (a private alias, since native 22 is taken by x86 `pipe`). Confirmed end-to-end: daimon's `--aarch64` cross-build succeeds and the binary serves under `qemu-aarch64`. CI's warn-on-detect allowlist for this symbol now matches nothing — kept for older pins. |
| [sandhi § daimon-server-max-conns](https://github.com/MacCracken/sandhi/blob/main/docs/development/issues/archive/2026-05-10-daimon-server-max-conns.md) | **Low** | 2026-05-10 | ✅ **RESOLVED both sides — corrected 2.1.7** | ⛔ This row read "Open upstream / no daimon-side action", which was wrong on both halves. Sandhi shipped the epoll-cooperative `max_conns` enforcement in **`sandhi_server_run_async` at 1.4.9** (hardened 1.4.10) and ARCHIVED its filing; daimon collapsed `serve_async` onto that call at **1.2.6** (`src/server.cyr:224-244`). Nothing is open on either side. The `"reserved for 0.8.0+"` text still in `lib/sandhi.cyr` is a stale doc comment on the **sync** options struct (`sandhi_server_run_opts`), not the async path daimon uses. |

---

## Tier 4 — ADRs (`docs/adr/`)

| File | Last touched | Status | Notes |
|---|---|---|---|
| `001-rust-to-cyrius-port.md` | 2026-04-13 | 📦 Frozen | Accepted (0.7.0). Rust → Cyrius port rationale. Historical record. |
| `002-synchronous-http.md` | 2026-04-13 | 📦 Frozen | Accepted, then partially superseded by 1.1.0 (async via lib/async.cyr) and again by 1.1.4 (sandhi adoption). The ADR's "invalid" note is captured in CLAUDE.md; re-read at v2.0 to decide whether to revise or supersede with a new ADR. |
| `003-security-audit-process.md` | 2026-04-13 | 📦 Frozen | Accepted (0.7.0). P(-1) + Work-Loop audit cadence. Verified by every release since; the rule holds. |

**ADR posture**: low decision-velocity. Only architecturally significant calls earn an ADR — minor decisions ride CHANGELOG + design comments. 1.1.4 sandhi adoption was a candidate but rode the CHANGELOG entry; the migration audit at `docs/audit/2026-04-27-sandhi-migration.md` carries the deep rationale. Re-evaluate at v2.0.0 cut.

---

## Tier 5 — Audit reports (`docs/audit/`)

Date-stamped, frozen by design. Each P(-1) hardening pass per CLAUDE.md cadence lands a new report — old reports stay verbatim as the historical record.

| File | Date | Status | Notes |
|---|---|---|---|
| `2026-04-13-security-audit.md` | 2026-04-13 | 📦 Frozen | 0.7.0 P(-1) security audit. 10 findings, 9 fixed at 0.7.0, VULN-007 gated. |
| `2026-04-27-sandhi-migration.md` | 2026-04-27 | 📦 Frozen | 1.1.4 sandhi adoption — VULN-001 strengthened, VULN-008 trade-off documented (sandhi's 30s SO_RCVTIMEO replaces no-timeout), 1.1.5 sandhi follow-ups (now 1.2.1 / 1.2.2). |

Next audit slot: at v1.3.0 cut, or sooner if a CVE pattern surfaces in daimon's parser surfaces (HTTP server via sandhi, JSON via lib/json, Unix-socket IPC wire protocol, /proc resource scrape paths). The CLAUDE.md cadence sets the trigger.

---

## Tier 6 — Guides (`docs/guides/`)

| File | Last touched | Status | Notes |
|---|---|---|---|
| `api.md` | 2026-07-03 | ✅ Fresh | 24-endpoint API reference. Verified current in the 1.3.0 pass — carries no cyrius-pin or version-tied example commands; no change needed. |
| `quickstart.md` | 2026-07-03 | ✅ Fresh | Prereq → cyrius 6.3.43; build block adds `cyrius lib sync` + `cyrius deps` (lib/ gitignored); 225-test count; 28-module structure. |

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
