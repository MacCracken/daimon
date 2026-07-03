---
name: Daimon Documentation Health
description: Living state of doc currency in the daimon repo — fresh / stale / archived / open-question, refreshed as docs are touched
type: state
---

# Documentation Health — daimon

> **Last refresh**: 2026-07-03 (1.3.0 arc — cyrius 6.3.43 toolchain bump, VERSION single-source-of-truth, full doc-currency pass; earlier-cycle context preserved below).
> **Refresh cadence**: when docs are touched, update the affected row. Full re-audit at each minor (1.2.x → 1.3.0) cut.
> **Scope**: this repo only (`daimon`) — root-level files plus the entire `docs/` tree.

This is a **ledger**, not a one-time audit. Rewrite-in-place as docs change. Pattern lifted from [agnosys/docs/doc-health.md](https://github.com/MacCracken/agnosys/blob/main/docs/doc-health.md) and [cyrius/docs/doc-health.md](https://github.com/MacCracken/cyrius/blob/main/docs/doc-health.md) — same buckets, daimon-shaped tiers.

Daimon is the AGNOS agent orchestrator — every consumer (hoosh, agnoshi, aethersafha, the agent fleet) depends on the HTTP API surface and the supervisor / scheduler / federation primitives. Stale endpoint docs propagate downstream, so doc currency carries weight even though the doc surface is modest today (~15 files).

---

## At a glance — 2026-07-03 inventory

**~15 markdown files** total (7 root + 8 under `docs/`). Bucket counts after the 1.3.0 doc-currency pass:

| Bucket | Count | What it means |
|---|---|---|
| ✅ **Fresh — refreshed in 1.3.0 cycle** | ~12 | CHANGELOG (1.3.0 entry), VERSION (1.3.0), CLAUDE.md (6.3.43 / sandhi 1.7.0 / sakshi 2.4.3 / sigil), README, CONTRIBUTING, architecture/overview.md, guides/quickstart.md, guides/api.md (verified current), BENCHMARKS.md (re-baselined under 6.3.43), roadmap, this file. |
| 🟡 **Stale — refresh in place** | 0 | Cleared in the 1.3.0 pass — the v1.2.x doc-refresh backlog (README / CONTRIBUTING / overview / quickstart / BENCHMARKS) is drained. |
| 🔵 **Probably evergreen** | 3 | `CODE_OF_CONDUCT.md`, `LICENSE`, `SECURITY.md`. No version-tied claims. Re-read pass annually. |
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

**Stale set:** cleared in the 1.3.0 pass. Next full re-audit at the v1.3.0 → v1.4.0 cut (or sooner if a subsystem doc drifts).

---

## Tier 1 — Root files

| File | Last touched | Status | Notes |
|---|---|---|---|
| `README.md` | 2026-07-03 | ✅ Fresh | Cyrius pin → 6.3.43; build block updated to `cyrius lib sync` + `cyrius deps` + explicit build; 225-test count. Binary-size / LOC figures left as-is. |
| `CHANGELOG.md` | 2026-05-10 | ✅ Fresh | Source of truth for shipped work. 1.2.0 entry covers toolchain bump, sakshi bump, CI/release modernization, /lib/ gitignored, lint-clean, fmt re-enabled. |
| `CLAUDE.md` | 2026-05-10 | ✅ Fresh | Durable rules. 1.2.0 pin refreshes: cyrius 6.3.43, sakshi 2.4.3. |
| `CONTRIBUTING.md` | 2026-07-03 | ✅ Fresh | Cyrius pin → 6.3.43; workflow updated (`cyrius lib sync` + `cyrius deps`, explicit build, `cyrius tests`); lib/ gitignored note; module-split note. |
| `SECURITY.md` | 2026-04-13 | 🔵 Evergreen | Supported-versions table + reporting policy. Reread at v1.3.0. |
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
| `roadmap.md` | 2026-05-10 | ✅ Fresh | 1.1.5 sandhi follow-ups rescoped to 1.2.1 / 1.2.2; "Future (v1.2.0+)" renamed to "Future (v1.3.0+)"; 1.2.0 ship marked complete. |

**Missing today (file in 1.2.x cleanup):**
- `development/state.md` — agnosys convention for the live volatile state file (pin / build sizes / test count / consumer table / recent releases / slot ledger). Daimon's roadmap.md partially covers this; consider splitting in 1.2.x if scope grows.
- `development/capability-map.md` — auto-generated per-module kernel-surface map. Daimon's surface is mostly userland (HTTP API + IPC over Unix sockets), so the security value is smaller than for agnosys; flag as nice-to-have, not P1.

**Tier — Engineering issues (upstream trackers)**

Daimon does not carry its own `docs/development/issues/` directory — blockers that need upstream fixes live in the upstream repo (cyrius / sandhi / sakshi / etc.) per the "blockers live where they're fixed" convention. Daimon's roadmap + CHANGELOG carry pointers + severity tags; the upstream tracker is the source of truth.

| Tracker | Severity | Filed | Status | Notes |
|---|---|---|---|---|
| [cyrius § daimon-async-aarch64-sys-epoll-wait](https://github.com/MacCracken/cyrius/blob/main/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md) | **P2** | 2026-05-10 | 🟢 Open upstream / daimon CI tolerant | `SYS_EPOLL_WAIT` undefined on aarch64 (lib/async.cyr × lib/syscalls_aarch64_linux.cyr). Blocks `--aarch64` cross-build. CI warn-on-detect; x86_64 unaffected. Close when upstream lands the arch-dispatch shim. Pinned in cyrius roadmap under `v5.10.x — Held`. |
| [sandhi § daimon-server-max-conns](https://github.com/MacCracken/sandhi/blob/main/docs/issues/2026-05-10-daimon-server-max-conns.md) | **Low** | 2026-05-10 | 🟢 Open upstream / no daimon-side action | Sandhi 1.7.0's `sandhi_server_options_max_conns` accepted-but-not-honored. Blocks daimon's `serve_async` collapse into `sandhi_server_run_opts`. No security impact — 1.2.2 closed async slowloris independently via `set_recv_timeout_ms`. Close when upstream wires worker-pool or epoll-cooperative enforcement. Pinned in sandhi roadmap under `Post-arc — wait-for-trigger`. |

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
