# Security Audit — 2026-04-13

Audit of daimon v0.7.0 (Cyrius port) against known CVE patterns and vulnerability classes affecting daemon/orchestrator software.

## Attack Surface

| Surface | Protocol | Exposure |
|---|---|---|
| HTTP API | TCP port 8090 | Network (configurable bind address) |
| Unix domain sockets | `{socket_dir}/{agent_id}.sock` | Local (filesystem permissions) |
| Agent process management | fork/exec/kill syscalls | Local (PID space) |
| Filesystem (memory store) | File I/O | Local (data_dir) |
| Bump allocator | Heap | Process-internal |

---

## Findings

### VULN-001: HTTP Request Smuggling via Incomplete Parsing (HIGH)

**CVE reference**: [CVE-2025-32094](https://www.akamai.com/blog/security/cve-2025-32094-http-request-smuggling), [CVE-2026-26365](https://www.akamai.com/blog/security-research/2026/feb/cve-2026-26365-incorrect-processing-connection-transfer-encoding), [CVE-2026-28369](https://www.thehackerwire.com/undertow-request-smuggling-flaw-cve-2026-28369/)

**Description**: Our HTTP parser (`http_parse_method`, `http_parse_path`, `http_parse_body`) is minimal — it does not validate `Content-Length`, `Transfer-Encoding`, or `Connection` headers. A reverse proxy fronting daimon could interpret request boundaries differently, enabling request smuggling.

**Affected code**: `src/main.cyr` — `http_parse_body()`, `http_route()`

**Current state**: The server uses `Connection: close` on all responses and reads a single request per connection, which mitigates smuggling for direct connections. However, if placed behind a proxy that reuses connections, the lack of `Content-Length` validation on inbound requests means the body parsing relies on finding `\r\n\r\n` without verifying declared length matches actual body.

**Severity**: HIGH if behind a proxy, LOW for direct connections.

**Remediation**:
1. Parse `Content-Length` header and use it to delimit body reads
2. Reject requests with both `Content-Length` and `Transfer-Encoding`
3. Reject requests with duplicate `Content-Length` values (per [CVE-2026-1525](https://www.resolvedsecurity.com/vulnerability-catalog/CVE-2026-1525))
4. Limit maximum header size (currently reads up to 8192 bytes total)

---

### VULN-002: CRLF Header Injection in Response (MEDIUM)

**CVE reference**: [CVE-2026-27810](https://cve.threatint.eu/CVE/CVE-2026-27810), [CWE-113](https://cwe.mitre.org/data/definitions/113.html)

**Description**: Several API handlers include user-controlled strings (agent names, task names, edge node names) directly in JSON response bodies without escaping. While JSON values are quoted, a crafted name containing `\"` could break out of the JSON string context. Additionally, the `api_rag_query` handler escapes `"`, `\n`, `\r`, `\` in the context string — but other handlers (agent name, task name, etc.) do not escape at all.

**Affected code**: `api_register_agent()`, `api_list_agents()`, `api_sched_submit()`, `api_edge_register()`, `api_sched_get_task()`, `api_edge_get()`

**Current state**: Names are inserted into JSON with `str_builder_add` between `\"` delimiters. A name containing `"` would break the JSON structure. A name containing `\r\n` in the HTTP header context could inject headers (though our CRLF generation uses `http_crlf()` which is separate from body content).

**Severity**: MEDIUM — JSON structure corruption; response splitting unlikely due to body being length-delimited.

**Remediation**:
1. Add a `json_escape_str()` function that escapes `"`, `\`, `\n`, `\r`, `\t` and control characters
2. Apply it to all user-controlled strings before embedding in JSON responses
3. Already done for RAG query context — generalize the pattern

---

### VULN-003: JSON Parsing Depth Not Limited (MEDIUM)

**CVE reference**: [CVE-2025-52999](https://www.herodevs.com/blog-posts/cve-2025-52999-denial-of-service-via-stack-overflow-in-jackson-core), [CVE-2026-29062](https://advisories.gitlab.com/pkg/maven/tools.jackson.core/jackson-core/CVE-2026-29062/)

**Description**: The `json_parse()` function in `lib/json.cyr` has no recursion depth limit. A deeply nested JSON payload could cause stack overflow.

**Current state**: Our JSON parser only handles flat objects (`{"key": "value"}`) — it does not recurse into nested objects or arrays. This means the depth-bombing attack vector does not apply to the current parser. However, if the parser is ever extended to handle nested JSON, this becomes a risk.

**Severity**: LOW (currently safe due to flat-only parser) — note for future.

**Remediation**: Document the flat-only constraint. If nested JSON support is added, enforce a max depth of 64.

---

### VULN-004: PID Reuse Race in Agent Signal Delivery (MEDIUM)

**CVE reference**: [CVE-2020-15702](https://flatt.tech/research/posts/race-condition-vulnerability-in-handling-of-pid-by-apport/), [LWN: Race-free process signaling](https://lwn.net/Articles/773459/)

**Description**: `agent_stop()`, `agent_pause()`, `agent_resume()` send signals via `syscall(SYS_KILL, pid, signal)`. Between checking the agent's PID and sending the signal, the process could exit and the PID could be reused by an unrelated process.

**Affected code**: `agent_stop()`, `agent_pause()`, `agent_resume()`

**Current state**: The window is small (check status → send signal), and daimon is single-threaded, but on a busy system with rapid PID cycling, the race is real.

**Severity**: MEDIUM — could kill an unrelated process.

**Remediation**:
1. Use `pidfd_open()` (syscall 434, Linux 5.3+) to obtain a stable file descriptor for the process at spawn time
2. Use `pidfd_send_signal()` (syscall 424) instead of `kill()` for signal delivery
3. Fall back to `kill()` on older kernels

---

### VULN-005: TOCTOU in Memory Store Atomic Write (LOW)

**CVE reference**: [CVE-2025-68146](https://advisories.gitlab.com/pkg/pypi/filelock/CVE-2025-68146/), [CVE-2026-22702](https://windowsforum.com/threads/toctou-in-virtualenv-cve-2026-22702-fixed-in-v20-36-1.402506/)

**Description**: `memory_store_set()` writes to a `.tmp` file then renames to the target. If the data directory is writable by other users, an attacker could create a symlink at the `.tmp` path pointing to a sensitive file, causing daimon to overwrite it.

**Affected code**: `memory_store_set()` — write to tmp, rename

**Current state**: The data directory (`/var/lib/agnos/agent-memory`) should be owned by the daimon user with restrictive permissions. The agent memory directory is created with `syscall(SYS_MKDIR, ..., 493)` (0755) which allows other users to read but not write — so symlink creation by non-owners is prevented.

**Severity**: LOW — mitigated by directory permissions, but not by O_NOFOLLOW.

**Remediation**:
1. Use `O_NOFOLLOW | O_CREAT | O_EXCL` flags when opening the tmp file to prevent symlink following
2. Create agent directories with mode 0700 instead of 0755
3. Verify parent directory ownership before writing

---

### VULN-006: Unix Socket Without Peer Credential Verification (MEDIUM)

**CVE reference**: [CVE-2026-21636](https://www.sentinelone.com/vulnerability-database/cve-2026-21636/), [CVE-2025-14282](https://forum.openwrt.org/t/security-advisory-2025-12-16-1-dropbear-privilege-escalation-via-unix-domain-socket-forwarding-cve-2025-14282/244222/1)

**Description**: `agent_ipc_accept_one()` accepts connections on Unix domain sockets without verifying the peer's credentials. Any local user who can connect to the socket can send messages.

**Affected code**: `agent_ipc_bind()`, `agent_ipc_accept_one()`

**Current state**: Sockets are created with mode 0700 via `syscall(SYS_CHMOD, ...)`, which restricts access to the socket owner. However, `SO_PEERCRED` (`getsockopt` with `SOL_SOCKET`/`SO_PEERCRED`) is not used to verify the connecting process's UID/GID.

**Severity**: MEDIUM — relies solely on filesystem permissions for access control.

**Remediation**:
1. After `accept()`, call `getsockopt(fd, SOL_SOCKET, SO_PEERCRED, ...)` to get peer UID
2. Verify peer UID matches expected agent UID or the daimon service UID
3. Reject connections from unexpected UIDs

---

### VULN-007: Bump Allocator Memory Reuse Information Leak (LOW)

**CVE reference**: [CVE-2026-34988](https://cvereports.com/reports/CVE-2026-34988)

**Description**: The cyrius bump allocator (`alloc()`) returns memory from a contiguous arena without zeroing it. If a previous allocation stored sensitive data (keys, tokens, agent payloads), a subsequent allocation at the same address could read stale data.

**Current state**: Daimon is a single-purpose daemon — all allocations belong to the same trust domain. Cross-tenant information leaks (as in CVE-2026-34988/Wasmtime) do not apply because there is no multi-tenant isolation within a single daimon process.

**Severity**: LOW — single trust domain, no sandboxing boundary.

**Remediation**:
1. Zero sensitive buffers before reuse (keys, auth tokens, IPC message payloads)
2. Consider using `memset(buf, 0, len)` on security-critical allocations
3. Document that bump allocator does not provide isolation between agents within the same process

---

### VULN-008: No Request Size Limit on HTTP Body (MEDIUM)

**Description**: The HTTP server reads up to 8192 bytes per request (`var req_buf = alloc(8192)`). However, a client can send a very large `Content-Length` header value, and the server does not validate it. The body parser scans for `\r\n\r\n` within the 8192-byte buffer, so oversized bodies are effectively truncated — but the client connection hangs until timeout.

**Current state**: The 8192-byte buffer acts as an implicit limit. POST bodies larger than ~8KB are silently truncated.

**Severity**: MEDIUM — potential DoS via slow-loris or oversized request holding connections.

**Remediation**:
1. Add explicit `MAX_REQUEST_SIZE` constant (e.g., 65536)
2. Read in a loop up to the declared Content-Length or max size
3. Return 413 Payload Too Large for oversized requests
4. Add a read timeout to prevent slow-loris attacks

---

### VULN-009: No Rate Limiting on API Endpoints (LOW)

**Description**: The HTTP server has no rate limiting. An attacker can flood any endpoint (especially `/v1/agents` POST, `/v1/scheduler/tasks` POST) to exhaust bump allocator memory or fill hashmaps.

**Severity**: LOW — single-threaded server limits throughput naturally, and bump allocator will eventually OOM.

**Remediation**:
1. Track per-IP request counts with a sliding window
2. Return 429 Too Many Requests when threshold exceeded
3. Add `max_agents` and `max_tasks` enforcement in submit handlers

---

### VULN-010: Agent Process Resource Limits Not Applied (LOW)

**Description**: The Rust version had `apply_rlimits()` using `setrlimit(RLIMIT_AS, RLIMIT_CPU)` in the `pre_exec` hook. The Cyrius port spawns agent processes via `spawn()` (fork+exec) but does not set resource limits on spawned processes.

**Severity**: LOW — agents can consume unlimited memory/CPU.

**Remediation**:
1. After `fork()` and before `exec()`, apply rlimits via `setrlimit` syscalls
2. Or use cgroups v2 via the kybernet library for resource isolation

---

## Summary

| ID | Severity | Status | Area |
|---|---|---|---|
| VULN-001 | HIGH | **Fixed** | HTTP parsing — Content-Length validation, Transfer-Encoding rejection (501), 413 Payload Too Large |
| VULN-002 | MEDIUM | **Fixed** | JSON response escaping — `json_escape_str()` on all user-controlled strings |
| VULN-003 | LOW | Mitigated | JSON parsing depth — flat parser, no recursion |
| VULN-004 | MEDIUM | **Fixed** | PID reuse race — `pidfd_open()`/`pidfd_send_signal()` with `kill()` fallback |
| VULN-005 | LOW | **Fixed** | File TOCTOU — agent memory dirs now 0700, not 0755 |
| VULN-006 | MEDIUM | **Fixed** | IPC auth — `SO_PEERCRED` UID verification on Unix socket accept |
| VULN-007 | LOW | Accepted risk | Memory reuse — single trust domain |
| VULN-008 | MEDIUM | **Fixed** | Request size — MAX_REQUEST_SIZE=65536, Content-Length body reads, 413 response |
| VULN-009 | LOW | **Fixed** | Rate limiting — per-IP 120 req/min sliding window, 429 Too Many Requests |
| VULN-010 | LOW | **Fixed** | Process rlimits — `agent_spawn_with_limits()` applies RLIMIT_AS + RLIMIT_CPU |

**Remediation status**: 9/10 fixed, 1 accepted risk (VULN-007 bump allocator — single trust domain)

## Sources

- [CVE-2025-32094: HTTP Request Smuggling via OPTIONS](https://www.akamai.com/blog/security/cve-2025-32094-http-request-smuggling)
- [CVE-2026-26365: Connection: Transfer-Encoding](https://www.akamai.com/blog/security-research/2026/feb/cve-2026-26365-incorrect-processing-connection-transfer-encoding)
- [CVE-2026-28369: Undertow Request Smuggling](https://www.thehackerwire.com/undertow-request-smuggling-flaw-cve-2026-28369/)
- [CVE-2026-1525: Duplicate Content-Length](https://www.resolvedsecurity.com/vulnerability-catalog/CVE-2026-1525)
- [CVE-2026-27810: CRLF Header Injection in calibre](https://cve.threatint.eu/CVE/CVE-2026-27810)
- [CWE-113: HTTP Response Splitting](https://cwe.mitre.org/data/definitions/113.html)
- [CVE-2025-52999: JSON Depth DoS](https://www.herodevs.com/blog-posts/cve-2025-52999-denial-of-service-via-stack-overflow-in-jackson-core)
- [CVE-2026-29062: Jackson Nesting Bypass](https://advisories.gitlab.com/pkg/maven/tools.jackson.core/jackson-core/CVE-2026-29062/)
- [CVE-2020-15702: PID Reuse Race](https://flatt.tech/research/posts/race-condition-vulnerability-in-handling-of-pid-by-apport/)
- [LWN: Race-free Process Signaling](https://lwn.net/Articles/773459/)
- [CVE-2025-68146: filelock TOCTOU Symlink](https://advisories.gitlab.com/pkg/pypi/filelock/CVE-2025-68146/)
- [CVE-2026-21636: Node.js UDS Permission Bypass](https://www.sentinelone.com/vulnerability-database/cve-2026-21636/)
- [CVE-2025-14282: Dropbear UDS Privilege Escalation](https://forum.openwrt.org/t/security-advisory-2025-12-16-1-dropbear-privilege-escalation-via-unix-domain-socket-forwarding-cve-2025-14282/244222/1)
- [CVE-2026-34988: Wasmtime Allocator Memory Leak](https://cvereports.com/reports/CVE-2026-34988)
- [CVE-2025-38236: Linux MSG_OOB Privilege Escalation](https://linuxsecurity.com/news/security-vulnerabilities/linux-kernel-bug-grants-attackers-full-kernel-level-control)
- [CVE-2025-9074: Docker Container Escape](https://thehackernews.com/2025/08/docker-fixes-cve-2025-9074-critical.html)
- [CVE-2026-34040: Docker AuthZ Bypass](https://thehackernews.com/2026/04/docker-cve-2026-34040-lets-attackers.html)
