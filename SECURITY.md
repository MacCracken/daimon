# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

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

This policy covers the `daimon` crate and its published API. Vulnerabilities in
upstream dependencies should be reported to the respective maintainers; we will
track and update our dependency pins as fixes become available.

Security concerns for this crate primarily involve:

- **Authentication/authorization bypass**: Accessing agent operations without proper credentials
- **Agent isolation**: Escaping sandbox or accessing other agents' data
- **Input validation**: Malformed HTTP requests causing crashes or exploitation
- **IPC security**: Unauthorized access to Unix domain sockets
- **Denial of service**: Inputs that cause excessive computation or memory usage
- **Dependency vulnerabilities**: Issues in upstream crates

We run `cargo audit` and `cargo deny` in CI to catch known dependency
vulnerabilities.

## Disclosure

We follow coordinated disclosure. Reporters will be credited in the release
notes unless they prefer to remain anonymous.
