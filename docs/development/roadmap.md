# Daimon Roadmap

> **Scope**: open work only. Shipped work is recorded in [CHANGELOG.md](../../CHANGELOG.md), not here.
>
> **Severity legend**: **P0** blocking (security / correctness — must-fix before ship) · **P1** high (must-have for the current arc) · **P2** medium (schedule when capacity opens) · **P3 / Low** nice-to-have, no urgency. Upstream-blocker items quote the upstream tracker's own severity.

**Current status** — `1.3.3` (development). Recent ships: 1.3.0 (cyrius 6.3.43 toolchain, VERSION single-source-of-truth, bote `libro_*` audit tools + audit-event feed), 1.3.1 (doc sweep), 1.3.2 (VULN-007 consumer-side secret-hygiene), 1.3.3 (distributed tracing over sakshi, `serve --trace`). No open feature work is scheduled on the current arc; v1.0 criteria are all met. See CHANGELOG for detail.

## Security Gates (trigger-based)

- [ ] **P0 (gated)** — **VULN-007: Bump allocator memory zeroing.** **MUST fix before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted federation, external MCP callbacks (bote). Two halves: (a) the **structural** fix is upstream — the cyrius bump allocator's `alloc_reset()` rewinds without zeroing the reclaimed span (filed [2026-07-03-cyrius-alloc-reset-no-zero-reused-memory.md](issues/2026-07-03-cyrius-alloc-reset-no-zero-reused-memory.md)); `lib/alloc.cyr` is vendored stdlib so daimon can't own it. (b) the **consumer-side** secret-hygiene layer shipped at **1.3.2** — `secure_zero` / `secure_zero_str` (`src/secmem.cyr`) + scrubbing the memory store's transient value buffers so sensitive agent data doesn't linger in the never-freed heap. Still open on the consumer side: **per-agent arena isolation**, the hard prerequisite before any multi-tenant / sandbox / untrusted-federation gate flips (secret-hygiene reduces exposure but does not isolate trust domains). Severity is **P0 when triggered**, dormant today because no consumer has flipped any of the gating conditions. Re-evaluate at every v1.x.0 cut.

## Blocked on Upstream Ports

- [ ] **Low (upstream nein)** — **Firewall MCP tools.** nein's Cyrius port has firewall / mesh / nat / netns ported but **no `mcp.cyr`** (the Rust `mcp.rs` is unported), so daimon can't wire firewall control through its MCP surface yet. No active consumer demand; bumps to **P2** when a consumer asks for it.

## Future (v1.4.0+)

Unsequenced; severity is assigned at the v1.4.0 cut once the arc's shape is chosen.

- [ ] jnana integration — grounded knowledge queries backed by verified AGNOS science data
- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Agent migration between nodes
