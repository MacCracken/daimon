# ADR-003: Security Audit Process

**Status**: Accepted
**Date**: 2026-04-13
**Context**: Daimon is a network-facing daemon managing agent processes with filesystem and IPC access. Security is not optional.

## Decision

Integrate external CVE/0-day research and formal audit reports into the development process.

## Process

### P(-1) Scaffold Hardening (steps 5-6)

1. **External security research** — Search for CVEs, 0-days, and vulnerability patterns relevant to daimon's attack surface: HTTP servers, Unix domain sockets, process supervisors, JSON parsers, file-based stores, bump allocators.
2. **Security audit report** — Write findings to `docs/audit/{date}-security-audit.md`. Each finding gets: severity, CVE references, affected code lines, current state assessment, and remediation steps.
3. **Roadmap** — Add repair work to `docs/development/roadmap.md`. Critical/High findings block the release. Medium findings go to next sprint. Low findings go to backlog or are accepted with documented rationale.

### Work Loop (step 6)

If security-touching changes were made (HTTP parsing, IPC, process management, file I/O, auth), repeat the external research + audit cycle for the affected subsystems.

### Security Gates

Some findings are acceptable under current conditions but become critical under future conditions. These are documented as "trigger-based" in the roadmap with explicit conditions that require remediation before proceeding.

## Consequences

- Every release has a traceable security audit in `docs/audit/`.
- CVE references tie findings to real-world exploitation patterns, not theoretical concerns.
- Security gates prevent feature creep from accidentally introducing vulnerabilities.
