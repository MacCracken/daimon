# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 2.4.x   | Yes       |
| < 2.4   | No        |

## Reporting a Vulnerability

Please report security vulnerabilities through
[GitHub Security Advisories](https://github.com/MacCracken/daimon/security/advisories/new).

**Do not** open a public issue for security vulnerabilities.

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact assessment

## Response Timeline

| Stage | Target |
|-------|--------|
| Acknowledgement | 48 hours |
| Initial assessment | 5 business days |
| Critical severity fix | 14 days |
| High severity fix | 30 days |
| Moderate/Low severity | Next scheduled release |

## Scope

This policy covers the daimon binary and its HTTP API. Security concerns include:

- **Authentication/authorization bypass**: Accessing agent operations without proper credentials
- **Agent isolation**: Escaping sandbox or accessing other agents' data
- **Input validation**: Malformed HTTP requests causing crashes or exploitation
- **IPC security**: an agent's channel (its fd 3, since 2.3.3) — frames that crash, stall or flood daimon's event loop, that are attributed to another agent, or that grow its memory
- **Browser reach**: requests a web page can make — DNS rebinding past the Host allowlist, cross-site writes past the Origin rules (2.3.4)
- **Agent containment** (2.4.2): a process an agent starts that outlives the agent's stop, or survives it exiting, where daimon reports the agent `"contained":true` (a cgroup of its own, ADR-008). Two cases are known and recorded. Where daimon reports `false`, a process that leaves the agent's process group is out of reach. And an agent runs as daimon's user, so one that writes its own processes into another cgroup that user may write takes them out: containment holds what detaches (`setsid`, a double fork, a daemon), not an agent acting against daimon.
- **Connection slots** (2.4.2): a way for one client to keep others from being served that the oldest-request-gives-way rule does not cover (a flood of new connections is known, and recorded)
- **On AGNOS** (2.4.0): daimon's agnos arms (spawn, signals, channels, its polled loop, and since 2.4.1 its detached calls). Known kernel gaps are listed, with daimon's interim for each, in `docs/audit/2026-09-23-agnos-platform-audit.md`: agents run without limits, any process can act on any TCP connection, the listener cannot be loopback-only, and a local client that stops reading a large answer, or on a slow machine one that falls behind, can stop the machine. A report that goes beyond them is in scope.
- **Denial of service**: Inputs that cause excessive computation or memory usage
- **Request smuggling**: Ambiguous HTTP parsing behind reverse proxies
- **Memory safety**: Buffer overflows, use-after-free, or information leaks via bump allocator

Security audit reports are published in `docs/audit/`.

## Disclosure

We follow coordinated disclosure. Reporters will be credited in the release
notes unless they prefer to remain anonymous.
