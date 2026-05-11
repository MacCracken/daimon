---
name: Daimon Documentation Health
description: Living state of doc currency in the daimon repo — fresh / stale / archived / open-question, refreshed as docs are touched
type: state
---

# Documentation Health — daimon

> **Last refresh**: 2026-05-10 (initial audit at 1.2.0 ship — paired with the cyrius 5.10.34 + sakshi 2.2.3 bump + CI/release modernization).
> **Refresh cadence**: when docs are touched, update the affected row. Full re-audit at each minor (1.2.x → 1.3.0) cut.
> **Scope**: this repo only (`daimon`) — root-level files plus the entire `docs/` tree.

This is a **ledger**, not a one-time audit. Rewrite-in-place as docs change. Pattern lifted from [agnosys/docs/doc-health.md](https://github.com/MacCracken/agnosys/blob/main/docs/doc-health.md) and [cyrius/docs/doc-health.md](https://github.com/MacCracken/cyrius/blob/main/docs/doc-health.md) — same buckets, daimon-shaped tiers.

Daimon is the AGNOS agent orchestrator — every consumer (hoosh, agnoshi, aethersafha, the agent fleet) depends on the HTTP API surface and the supervisor / scheduler / federation primitives. Stale endpoint docs propagate downstream, so doc currency carries weight even though the doc surface is modest today (~15 files).

---

## At a glance — 2026-05-10 inventory

**~15 markdown files** total (7 root + 8 under `docs/`). Bucket counts after the 1.2.0 ship:

| Bucket | Count | What it means |
|---|---|---|
| ✅ **Fresh — touched in 1.2.0 cycle** | ~8 | CHANGELOG (1.2.0 entry), VERSION, CLAUDE.md (cyrius pin + sakshi pin refresh), roadmap (v1.2.x carryovers, future deferrals), this file. |
| 🟡 **Stale — refresh in place** | ~5 | README.md (toolchain block + footprint numbers), CONTRIBUTING.md (cyrius pin + CI gate list), architecture/overview.md (sandhi 1.3.x + tls dep additions), BENCHMARKS.md (last numbers at 1.1.x), guides/quickstart.md (cyrius install command). Sequence into 1.2.1+. |
| 🔵 **Probably evergreen** | 3 | `CODE_OF_CONDUCT.md`, `LICENSE`, `SECURITY.md`. No version-tied claims. Re-read pass annually. |
| 📦 **Archive / frozen by design** | ~4 | The 3 ADRs (point-in-time decisions); audit/2026-04-13 + audit/2026-04-27 reports (frozen by audit convention). |
| ❓ **Open strategic question** | 0 | None outstanding. See [Open questions](#open-strategic-questions) below for what would re-open it. |

**Doc work shipped in 1.2.0:**
- ✅ `CHANGELOG.md` — 1.2.0 entry recording the cyrius 5.10.34 / sakshi 2.2.3 bump + CI/release rewrite + `/lib/` gitignored.
- ✅ `CLAUDE.md` — cyrius pin reference refreshed 5.7.12 → 5.10.34; sakshi line refreshed 2.0.0 → 2.2.3; sandhi note remains "in use".
- ✅ `docs/development/roadmap.md` — 1.1.5 items rescoped to 1.2.1 / 1.2.2; "Future (v1.2.0+)" renamed to "Future (v1.3.0+)".
- ✅ `docs/doc-health.md` — this file (initial scaffold; agnosys convention).

**Stale set carried into 1.2.1+:** the README footprint block, CONTRIBUTING workflow steps + cyrius pin, architecture overview's deps list, BENCHMARKS numbers, quickstart install command. None block 1.2.0 ship — all are read-through refreshes, batched into a 1.2.1 doc cleanup pass per the working-loop convention.

---

## Tier 1 — Root files

| File | Last touched | Status | Notes |
|---|---|---|---|
| `README.md` | (pre-1.2.0) | 🟡 Stale | Cyrius pin, binary size, dependency list, build commands all reference the 1.1.x state. Refresh in 1.2.1: cyrius 5.10.34, sakshi 2.2.3, binary ~622 KB (was 452 KB), stdlib dep list (added tls/mmap/dynlib/fdlopen for sandhi 1.3.3). |
| `CHANGELOG.md` | 2026-05-10 | ✅ Fresh | Source of truth for shipped work. 1.2.0 entry covers toolchain bump, sakshi bump, CI/release modernization, /lib/ gitignored, lint-clean, fmt re-enabled. |
| `CLAUDE.md` | 2026-05-10 | ✅ Fresh | Durable rules. 1.2.0 pin refreshes: cyrius 5.10.34, sakshi 2.2.3. |
| `CONTRIBUTING.md` | (pre-1.2.0) | 🟡 Stale | Refresh in 1.2.1: cyrius pin, CI gate list (vet, fmt-via-diff, lint fail-on-warn, security scan, docs checks), `cyrius deps` workflow, lib/ gitignore expectation. |
| `SECURITY.md` | 2026-04-13 | 🔵 Evergreen | Supported-versions table + reporting policy. Reread at v1.3.0. |
| `CODE_OF_CONDUCT.md` | (initial) | 🔵 Evergreen | Standard. |
| `BENCHMARKS.md` | (pre-1.2.0) | 🟡 Stale | Last numbers captured against Rust at 1.0.x. Re-baseline under 5.10.34 in 1.2.1 (numbers should be within noise — none of the 16 microbenchmarks touch HTTP). |
| `VERSION` | 2026-05-10 | ✅ Fresh | `1.2.0` — single source of truth, read into `cyrius.cyml` via `${file:VERSION}`. |
| `LICENSE` | (initial) | 🔵 Evergreen | GPL-3.0-only. |

---

## Tier 2 — Architecture (`docs/architecture/`)

| File | Last touched | Status | Notes |
|---|---|---|---|
| `overview.md` | (pre-1.2.0) | 🟡 Stale | Module map + data flow + consumers. Refresh in 1.2.1: stdlib deps list (added tls/mmap/dynlib/fdlopen for sandhi 1.3.3 unconditional TLS_EARLY_DATA refs); cyrius pin; lib/ gitignored note; sandhi 1.3.3 client-side 0-RTT mention (daimon doesn't use it — server only — but the deps are required for the bundle to compile). |

---

## Tier 3 — Project state (`docs/development/`)

| File | Last touched | Status | Notes |
|---|---|---|---|
| `roadmap.md` | 2026-05-10 | ✅ Fresh | 1.1.5 sandhi follow-ups rescoped to 1.2.1 / 1.2.2; "Future (v1.2.0+)" renamed to "Future (v1.3.0+)"; 1.2.0 ship marked complete. |

**Missing today (file in 1.2.x cleanup):**
- `development/state.md` — agnosys convention for the live volatile state file (pin / build sizes / test count / consumer table / recent releases / slot ledger). Daimon's roadmap.md partially covers this; consider splitting in 1.2.x if scope grows.
- `development/capability-map.md` — auto-generated per-module kernel-surface map. Daimon's surface is mostly userland (HTTP API + IPC over Unix sockets), so the security value is smaller than for agnosys; flag as nice-to-have, not P1.

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
| `api.md` | (pre-1.2.0) | 🟡 Stale | 24-endpoint API reference. No public-surface changes in 1.2.0 (toolchain bump only), but the cyrius pin and example commands need a refresh; do at 1.2.1 alongside README. |
| `quickstart.md` | (pre-1.2.0) | 🟡 Stale | Install command references the 5.7.12 / sakshi 2.0.0 era. Refresh in 1.2.1: cyrius 5.10.34 install one-liner (versioned layout), sakshi 2.2.3 tag, `cyrius deps` workflow (lib/ is gitignored). |

---

## Open strategic questions

None outstanding. The following are tracked elsewhere (issue tickets / roadmap), not strategic questions:

- **External MCP forwarding** via `sandhi_rpc_mcp_call` — sequenced into 1.2.1 per the working-loop discipline. Replaces the `api_mcp_call` "tool dispatch not available in sync mode" stub.
- **Sandhi `idle_ms` tuning + `serve_async` collapse** — sequenced into 1.2.2; predicated on whether stdlib `sandhi_server_options_max_conns` enforcement landed by 5.10.34 (premise-check first).
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
