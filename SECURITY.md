# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 1.0.x   | Yes       |

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
- **IPC security**: Unauthorized access to Unix domain sockets
- **Denial of service**: Inputs that cause excessive computation or memory usage
- **Request smuggling**: Ambiguous HTTP parsing behind reverse proxies
- **Memory safety**: Buffer overflows, use-after-free, or information leaks via bump allocator

Security audit reports are published in `docs/audit/`.

## Disclosure

We follow coordinated disclosure. Reporters will be credited in the release
notes unless they prefer to remain anonymous.
