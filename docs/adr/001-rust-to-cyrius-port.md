# ADR-001: Rust to Cyrius Port

**Status**: Accepted
**Date**: 2026-04-13
**Context**: Daimon v0.6.0 was implemented in Rust with 193 crate dependencies, 9,724 LOC, and a 4.0 MB binary.

## Decision

Port daimon from Rust to Cyrius, the AGNOS sovereign systems language.

## Rationale

1. **Sovereignty** — Cyrius is self-hosting with zero external dependencies. The entire toolchain (seed + compiler + assembler) is ~380 KB. No LLVM, no libc in the bootstrap path.
2. **Binary size** — 4.0 MB (Rust) → 181 KB (Cyrius). 96% reduction. Critical for edge fleet deployment where bandwidth and storage are constrained.
3. **Dependency elimination** — 193 crates → 0 external dependencies. No supply chain attack surface. No `cargo audit` needed.
4. **Build speed** — Full compile in <100ms vs multi-minute Rust builds. No 16 GB target directory.
5. **Ecosystem alignment** — Daimon is the AGNOS agent orchestrator. AGNOS runs on Cyrius. The orchestrator should speak the same language as its agents.

## Trade-offs

- **Performance**: 1.5-60x slower on tight loops due to single-pass compilation (no LLVM optimizations, no SIMD auto-vectorization). Wins on allocation-heavy workloads (3.5-42x faster via bump allocator).
- **Async**: Synchronous HTTP server. No concurrent request handling. Acceptable for orchestrator workloads; async deferred to Cyrius stdlib maturity.
- **Type safety**: Everything is i64. Manual struct layout. No borrow checker. Mitigated by comprehensive test suite (200 assertions) and fuzz harnesses (5).
- **Blocked features**: Firewall (nein) and MCP hosting (bote) require upstream Cyrius ports.

## Consequences

- Rust source removed from repo in v1.0.1. Available in git history (pre-v0.7.0 tags).
- All development uses `cyrius build`, `cyrius check`, `.cyrius-toolchain` pinning.
- Security audit process added to development workflow (P(-1) step 5-6, work loop step 6).
