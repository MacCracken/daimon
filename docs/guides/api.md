# HTTP API Guide

Daimon exposes a REST API on `127.0.0.1:8090`. Set the port with `serve [port]` and the bind
address with `serve --listen ADDR`. Through 2.2.3 daimon bound every interface; since 2.3.0 it binds
loopback unless told otherwise. `--listen 0.0.0.0` opens every interface. The API has no
authentication, so do that only behind a firewall.

Answers are JSON, except the few refusals that have no body (see Error Responses). POST bodies are
JSON. The connection is closed after each answer.

**Who may call it** (2.3.4, VULN-012):
- While daimon listens on loopback (the default), a request must name a loopback host (`Host:
  127.0.0.1`, `localhost` or `[::1]`, any port); any other name gets **403**. This is what defeats
  DNS rebinding, where a web page's requests reach 127.0.0.1 under the attacker's hostname. With
  `--listen` beyond loopback the check is off: the operator chose the names daimon answers to.
- A request that changes state (POST, PUT, DELETE) and carries an `Origin` from another site gets
  **403** on every route. Such a request is what a web page sends with no CORS preflight (a
  `text/plain` POST). A page served from a loopback host (`http://127.0.0.1:…`, `http://localhost:…`)
  may still write. Agent and task control refuse any `Origin`, as since 2.3.0. Clients that are not
  browsers send no `Origin`.

**One client does not hold the server** (2.3.4): daimon runs its own event loop and reads requests
without blocking, so a slow client holds only its own connection:
- a connection that sends nothing for 5 s, or has no whole request after 30 s, is closed;
- at most 128 connections are open at once (5 on AGNOS), and more wait in the kernel's queue;
- when all 128 are taken and another is waiting, the oldest request still arriving gives way, once
  it is 1 s old (2.4.2). It is closed unanswered and counted (`http_evicted`). One client trickling
  in 128 requests held every slot for 30 s before.

## Request bodies

Since 2.3.2 a body is **one JSON object**, read with a full JSON parser. It gets **400** when it:
- is not JSON (for example form-encoded), has trailing content, or is not an object;
- repeats a key, in any object at any depth (at the top level since 2.3.2, nested ones since 2.5.1,
  which bayan 1.11.1 refuses). Parsers disagree on which one wins, so daimon accepts neither;
- has a number with a leading zero, such as `01` (2.5.1, bayan 1.11.1; RFC 8259 §6);
- has U+0000 in a top-level string.

Values:
- **Strings are decoded**: `"say \"hi\" \\o/ \u00e9"` is stored, and echoed back, as
  `say "hi" \o/ é`. Before 2.3.2 the escapes were kept as typed.
- **A string field** takes a JSON string, or a number read as its text (`"agent_id": 3` is `"3"`).
- **An integer field** takes a number, a numeric string (`"priority": "7"`), or true / false as 1 / 0.
- **Only top-level fields are read.** A key inside a nested object never stands in for a top-level
  one. Nested values that daimon forwards, such as MCP `arguments`, are passed on untouched.

## Distributed tracing

When started with `serve --trace`, daimon participates in a distributed trace on
every request:

- **Inbound** — it adopts the full 128-bit trace-id from a W3C `traceparent`
  header (or an `X-Trace-Id` header: 32-hex = 128-bit, 16-hex = 64-bit),
  generating one if neither is present.
- **Response** — it echoes `X-Trace-Id: <32 hex>` (the full trace-id) and emits
  timed sakshi spans (`http.request`, `mcp.builtin` / `mcp.forward`) correlated
  under that id.
- **Outbound** — on an external MCP forward it propagates the trace context
  downstream as a fresh `traceparent` request header, so that endpoint joins the
  trace.

```
curl -i -H "traceparent: 00-<32hex trace-id>-<16hex span-id>-01" http://127.0.0.1:8090/v1/health
→ ... X-Trace-Id: <32-hex trace-id>
```

Tracing is off by default (no `X-Trace-Id`, no span emission).

## Health

```
GET /v1/health
→ {"status":"ok","agents":0,"mcp_tools":13,"mcp_resources":0,"mcp_prompts":0,"edge_nodes":0}
```

`mcp_tools` counts the 13 builtins as well as registered tools.

## Agents

```
# List agents
GET /v1/agents
→ {"agents":[{"id":1,"name":"my-agent","type":"User","status":0,"pid":0,"exit_code":null}],"count":1}

# Register agent — type is optional: System, User (default) or Service
POST /v1/agents
{"name":"my-agent","type":"User"}
→ 201 {"id":1,"name":"my-agent","type":"User","status":0}
→ 400 unknown type · 409 at max_agents (1000)

# Get agent
GET /v1/agents/1
→ {"id":1,"name":"my-agent","type":"User","status":0,"pid":0,"exit_code":null}

# Start — runs the agent's process (2.3.0)
POST /v1/agents/1/start
→ {"id":1,"name":"my-agent","type":"User","status":2,"pid":4242,"exit_code":null,"limits_enforced":true,"contained":true}
→ 409 not Pending/Stopped/Failed · 422 no executable for its type (on AGNOS also: a name with a space)

# Stop — SIGTERM, up to 5 s to exit, then SIGKILL; always 200, once the agent is gone
POST /v1/agents/1/stop
→ {"id":1,"name":"my-agent","type":"User","status":5,"pid":0,"exit_code":0}

# Pause / resume — SIGSTOP / SIGCONT
POST /v1/agents/1/pause      → status 3 · 409 unless Running
POST /v1/agents/1/resume     → status 2 · 409 unless Paused

# Delete — refused while the agent has a process
DELETE /v1/agents/1
→ {"ok":true} · 409 while it has a process

# Messages (2.3.4) — queue one for the agent; take (and remove) what is queued
POST /v1/agents/1/messages            {"type":"command","payload":{...}}   → 201 {"id":13}
POST /v1/agents/1/messages/take       {"max":10}                           → {"messages":[...],"count":1,"remaining":0}

# Captured output (2.3.4; serve --agent-output capture) — stdout and stderr since its latest start
GET /v1/agents/1/output
→ {"output":"hello\n","bytes":6,"kept":6} · 409 when output is not captured
```

Agent status values: 0=Pending, 1=Starting, 2=Running, 3=Paused, 4=Stopping, 5=Stopped, 6=Failed.
A process that exits on its own is collected within 100 ms (daimon's upkeep tick, 2.3.4). Exit 0 leaves the agent
Stopped; a non-zero exit, or death by a signal, leaves it Failed. A stop through the API leaves it
Stopped, whatever its exit code (143 when the SIGTERM killed it). A Failed agent can be started
again. `exit_code` is null until a process has exited, and 128 + the signal number if a signal
killed it (137 = SIGKILL).

**What runs.** The request never names an executable. daimon runs `agnos-agent-<type>-agent`
(`system`, `user` or `service`) from `/usr/lib/agnos/agents`, then `/opt/agnos/agents`, else
`/usr/bin/agnos-agent-runner`. The argv is `--agent-id <id> --agent-name <name>`. The operator can
replace the search with `serve --agents-dir DIR`, where the runner is `DIR/agnos-agent-runner`. The
child:
- has stdin from `/dev/null`;
- inherits no descriptor above stderr except **fd 3, its channel to daimon** (2.3.3);
- starts with SIGPIPE at its default;
- gets daimon's environment, plus `AGNOS_IPC_FD=3`;
- runs under its supervisor quota as rlimits: 1 GiB address space and 3600 s CPU by default. A
  limit that cannot be applied refuses the start with a 500.

**A stop does not hold the server** (2.3.4, VULN-014). Its answer comes when the agent is gone, up
to about 6 s for an agent that ignores SIGTERM. Other requests, and agents' channels, are served
meanwhile. A GET in between shows the agent at status 4 (Stopping). A stop, pause or resume reaches
the processes the agent started too: each agent leads its own process group. An agent is killed if
daimon itself dies (`PR_SET_PDEATHSIG`); daimon's registry is in memory, so it could never be
reached again.

**Containment** (2.4.2, [ADR-008](../adr/008-agent-containment-cgroup.md)). A process that calls
`setsid` leaves its agent's process group, and through 2.4.1 it outlived the agent's stop. Where
daimon's own cgroup is its to divide, each agent runs in a cgroup of its own, and its JSON says
`"contained":true` while it has a process. That means a systemd service with `Delegate=yes`, anything
the user's systemd runs (`systemd-run --user`), or a cgroup handed to daimon's user. Then:
- a stop, pause or resume reaches every process in the cgroup;
- when the agent's own process is collected, anything left in it gets SIGTERM, 5 s, then SIGKILL.
  **An agent's processes end with it**. Its cgroup is then removed, with any the agent made inside
  it;
- a daimon that was killed outright left its agents' leftovers running, and the next daimon started
  in the same cgroup ends them.

This holds processes that detach (`setsid`, a double fork, a daemon). It does not hold an agent that
acts against daimon: an agent runs as daimon's user, and may move its own processes into another
cgroup that user may write.

Elsewhere, agents run as before, and `"contained":false`. That covers daimon started from a login
shell (the session's cgroup is root's), and a cgroup whose `cgroup.procs` daimon may not write.
daimon warns once at startup, and audits `agent.cgroup.unavailable`.

**Environment and output** (2.3.4):
- `serve --agent-env minimal` gives agents only PATH, HOME, USER, LOGNAME, SHELL, TERM, TMPDIR, TZ,
  LANG, LANGUAGE, LC_*, XDG_RUNTIME_DIR and AGNOS_*, instead of daimon's whole environment (the
  default, `inherit`; VULN-015).
- `serve --agent-output capture` collects each agent's stdout and stderr: the last 32 KiB, served
  at `GET /v1/agents/{id}/output`. Invalid UTF-8 is shown as U+FFFD. The default, `inherit`, leaves
  them daimon's own.

**On AGNOS** (2.4.0, [ADR-007](../adr/007-daimon-on-agnos.md)) the same routes use the agnos
kernel's primitives. Some things differ until agnos closes the gaps filed with it
(`docs/development/issues/2026-09-23-*.md` in the agnos repo):
- **Room**: the machine has 16 process slots and 16 channels. Measured in daimon's guest test,
  12 agents run at once. A start with no free channel, process slot or memory answers **503** ("no
  room for another agent"), and the agent stays as it was (2.4.1; a full process table since
  2.5.2, which agnos 1.57.6 reports). A start that cannot load the executable answers **500**.
- **Start**: `spawn_path` runs `<exe> --agent-id <id> --agent-name <name>` with each argument
  passed as it is (2.5.2), so a name may hold spaces. The arguments may take 1024 bytes in all; a
  longer name answers **422**. Through 2.5.1 they were one line split on spaces, at most 127 bytes.
  The environment is daimon's (filtered by `--agent-env`), at most 16 entries and 1024 bytes, plus
  `AGNOS_IPC_FD` naming the channel's fd, which the kernel chooses.
- **No limits**: agnos has none, so the agent runs without its quota. Its JSON says
  `"limits_enforced":false`, and each start is audited `agent.limits.unenforced`.
- **Stop**: SIGTERM, which the agent sees on a signalfd, then SIGKILL after the grace period, as on
  Linux. Since agnos 1.57.7 the SIGKILL ends an agent that ignores SIGTERM: status 5 (Stopped), exit
  code 137. Through 1.57.6 it stayed at status 4 (Stopping). Pause and resume answer **501**.
- **No captured output**: `serve --agent-output capture` exits 1, because an agnos child inherits
  daimon's whole fd table.
- **The listener** is on 127.0.0.1, loopback only, as on Linux (2.4.4). That needs agnos 1.57.7 or
  later: an older kernel refuses it, and daimon says `cannot listen on 127.0.0.1:8090` and exits 1.
  `--listen 0.0.0.0` listens on every address. Through 2.4.3 the listener took the NIC's address.
- **5 connections at once**, not 128. The kernel has 8 TCP slots for the whole machine: the
  listener takes one, and daimon's own calls to other servers need theirs.
- **The rate limit is shared.** agnos gives no peer address, so every client draws on one bucket of
  960 requests a minute (120 for each of 8 connections).
- **Answers are written 512 bytes at a time, 1 ms apart** (2.4.1; 512 bytes and the 1 ms since
  2.4.3, where it had been 1 KB and one yield). A TCP receive ring on agnos is 2 KB, and the
  kernel holds the CPU while a send waits for room, so a write a local client has no room for stops
  the machine. A client that reads keeps up. One that stops reading mid-answer can still stop it, and
  so, rarely, can one on a very slow machine. An answer goes at 512 KB/s at most.

**Browsers may not control agents.** start / stop / pause / resume / DELETE answer **403** to any
request carrying an `Origin` header. A web page can send a cross-origin `text/plain` POST with no
CORS preflight, and could otherwise drive agents from a browser (2.3.0 audit, VULN-012). curl,
agents and other native clients send no `Origin`.

**Agents talk back on fd 3** (2.3.3). A started agent writes length-prefixed JSON frames to its
channel and gets a one-byte reply for each. daimon puts accepted messages on its message bus, where
each registered agent has a queue. The wire format, the limits and what closes a channel are in
[agent-ipc.md](agent-ipc.md). The channel's traffic shows in `/v1/metrics`.

## MCP

daimon is an MCP host. It lists and calls its own 13 builtin tools, and the tools, resources and
prompts that external MCP servers register with it. A call to an external one is forwarded to the
server's `callback_url` as JSON-RPC 2.0 (`tools/call`, `resources/read`, `prompts/get`), with the
request body as its params.

### Built-in tools

| tools | what they do |
|---|---|
| `libro_query`, `libro_verify`, `libro_export`, `libro_proof` | daimon's audit chain (libro's tools, through bote): search it, verify its hash links, export it, prove an entry's inclusion |
| `libro_retention` | libro's retention policy, which archives entries off the chain. **Refused** until callers are authenticated (2.5.1; roadmap 2.6.x): `"access denied: tool gated by host policy"`, audited `libro.retention.deny`. Any local caller could empty the audit chain with it |
| `web_fetch`, `web_search` | bote's web tools: a page's readable text (`{url}`, http or https only), and results from the SearXNG instance at `BOTE_SEARXNG_URL` (`{query, count?}`). Unset, `web_search` says so |
| `nein_validate` | nein's offline check of a rule (`{protocol, port, source?, table?, chain?}`); touches nothing outside daimon. Since nein 1.9.1 (daimon 2.5.3) a `table` or `chain` nft cannot read, a keyword such as `fwd` or a name starting with a digit, answers `invalid 'table' name` / `invalid 'chain' name` |
| `nein_status`, `nein_list`, `nein_diff` | nein's tools that read the live firewall: they run `nft` as root and return the ruleset. **Refused** until callers are authenticated (2.5.2; roadmap 2.6.x), audited `nein.gate.deny`. 2.1.8 – 2.5.1 answered them to any caller |
| `nein_allow`, `nein_deny` | nein's tools that change the live firewall. **Refused** until callers are authenticated (roadmap 2.6.x): `"access denied: tool gated by host policy"`, audited `nein.gate.deny` |

Every builtin answers with an MCP tool result, built by bote or nein and passed through as it is.
A libro tool's success carries its JSON twice, as the text of one content block and as
`structuredContent`; its error, since bote 3.8.5 (daimon 2.5.3), only the text block, with
`"isError":true`. Every nein tool but `nein_validate` runs `nft`, so it needs root and nftables.
Without them, or on AGNOS, which has no nftables, it answers `"isError":true`. No external
registration may take a builtin's name (409).

### Tools

```
# List tools, the builtins included (inputSchema is raw JSON Schema; {} / {"type":"object"} when unset).
# GET /v1/mcp/manifest answers the same. Each entry carries its digest and pin: see "Tool definition pins".
GET /v1/mcp/tools
→ {"tools":[{"name":"libro_verify","description":"Verify the libro chain's hash-link integrity",
    "inputSchema":{"type":"object"},"definition_sha256":"<64 hex>","pin_state":"pinned",
    "pinned_sha256":"<64 hex>","pinned_at":1793990381,"pin_from_store":true},...],
   "count":13,"pins":{"state":"loaded","durable":true,"count":14,"unpinned":0}}

# Register an external tool. inputSchema is optional (alias: input_schema), stored in bayan's compact
# form, {} when omitted. callback_url must be http:// or https:// (a scheme check, not a host allowlist).
POST /v1/mcp/tools
{"name":"scan","description":"port scanner","callback_url":"http://localhost:9000",
 "inputSchema":{"type":"object","properties":{"target":{"type":"string"}},"required":["target"]}}
→ 201 {"ok":true,"pin_state":"pinned"} · 400 no name or callback_url, or not http(s) · 409 a builtin's name
# Registering a name again replaces what is served, whoever registered it first (roadmap 2.6.x). It does
# not move the pin: {"ok":true,"pin_state":"changed"}, audited mcp.tool.changed.

# Call a tool: MCP tools/call params
POST /v1/mcp/call
{"name":"libro_verify","arguments":{}}
→ {"content":[{"type":"text","text":"{\"ok\":true}"}],"structuredContent":{"ok":true}}
{"name":"scan","arguments":{"target":"10.0.0.1"}}
→ the external server's result, verbatim
→ 400 unknown tool · 502 the server could not be reached, or answered wrongly · 504 no answer in 60 s

# Deregister
DELETE /v1/mcp/tools/scan
→ {"ok":true}
```

### Tool definition pins (2.5.4)

A tool's description is read by a model as guidance, so swapping one after it was approved is prompt
injection with an approval already granted. A client that pins definitions (thoth does) can rebuild
from daimon if it loses its own record, with one `GET /v1/mcp/tools`.

| field | what it says |
|---|---|
| `definition_sha256` | the digest of what this entry serves |
| `pinned_sha256` | the digest daimon first saw under this name |
| `pinned_at` | when, in epoch seconds |
| `pin_state` | `pinned` they match, `changed` they differ, `unpinned` no pin could be kept |
| `pin_from_store` | the pin came from a previous run, not a first sight in this one |

**The digest** is SHA-256, in lowercase hex, over

    name \0 description \0 inputSchema

with `name` and `description` decoded from their JSON strings, and `inputSchema` the text the manifest
serves. That text is compact (no whitespace), in the key order it was registered with, and parsing it
and re-serialising it compactly gives the same bytes. Neither name nor description can hold a NUL
(`400`). For example:

```bash
printf 'libro_verify\0Verify the libro chain'"'"'s hash-link integrity\0{"type":"object"}' | sha256sum
```

A pin is what daimon first saw, and **a registration cannot move it**. A definition that differs is
served, marked `changed`, and audited `mcp.tool.changed`. What to do with it is the client's decision.
Deregistering keeps the pin. A pin is kept in `<data_dir>/mcp/toolpins`, so it outlives a restart;
a registration does not, and a server registers again.

Two things move a pin: a new binary, for a builtin (audited `mcp.pin.builtin.moved`), and an operator:

```
POST /v1/mcp/tools/{name}/accept
→ 403 until callers are identified (roadmap 2.6.x), audited mcp.pin.accept.deny
→ with `serve --mcp-pin-accept`: {"ok":true,"pin_state":"pinned"} · 404 · 409 a builtin, or no pin
```

**A consumer accepts for itself** (2.5.5), which moves no pin. The operator issues it a token, shown
once. Only the token's digest is kept, and `remove` stops it at once, with no restart:

```bash
daimon consumer add thoth --data-dir /var/lib/agnos
```

```
# Name the digest you reviewed: a definition that has changed since answers 409.
POST /v1/mcp/tools/{name}/accept     Authorization: Bearer <token>
{"definition_sha256":"<64 hex>"}
→ {"ok":true,"pin_state":"accepted"} · 400 · 401 a token that names nobody · 403 t-ron refused · 404 · 409

# With the token, a manifest says what you accepted.
GET /v1/mcp/tools                    Authorization: Bearer <token>
→ entries add "accepted_sha256" and "accepted_at"; "pin_state":"accepted" where it is what is served
```

`accepted` reaches that consumer alone: the same tool reads `changed` for everyone else. A further swap
reads `changed` again.

**Every accept passes t-ron**, and only an allow passes. Name a policy with `serve --tron-policy FILE`: with
none, no consumer may accept (403), and a file t-ron refuses stops `serve`. t-ron denies by default a
consumer or a tool the policy does not name, and the action it judges is `mcp_accept.<tool>`:

```toml
[agent."thoth"]
allow = ["mcp_accept.*"]
deny = ["mcp_accept.firewall_*"]
```

t-ron also reads the definition, so a description it takes for a prompt injection, or a definition carrying
SQL, shell, template or path injection, answers 403 with the reason, whatever the policy allows.

`pins.state` in the manifest says how far to trust all this. `durable` is false in the last two:

| state | meaning |
|---|---|
| `loaded` | a previous run's records were read |
| `new` | none existed: every pin is this run's first sight |
| `rejected` | the file failed its checks. It is left untouched, nothing writes over it, and pins last for this run. An operator moves it aside to start again |
| `unavailable` | nothing can be kept (no data directory, or a write failed). On AGNOS a store needs 1.57.17 or later |

⚠ This is trust on first use, and it trusts daimon at the moment of the repair. It closes a swap by
an MCP server behind daimon, not one by daimon. Keep the client's own pins as well. See ADR-009.

### Forwarded calls

A call that waits on another server runs in a child process, so daimon keeps serving meanwhile.
That covers a forwarded call, `resources/read`, `prompts/get`, `web_fetch` and `web_search`. One
that has not finished within 60 s is answered 504 (2.3.4; on AGNOS since 2.4.1). On AGNOS a server
that sends nothing for 30 s is taken to have closed (502). Through 2.4.1 that was about a second
whenever the guest was busy (2.4.2). A JSON-RPC error from the server answers 200 with
`"isError":true` and its `code`.

### Resources and prompts (2.1.0)

A resource URI contains `://` and more slashes, so it travels in the body, never in the path.

```
# Register an external resource (mimeType, or mime_type; text/plain when omitted)
POST /v1/mcp/resources
{"uri":"file:///etc/motd","name":"motd","description":"message of the day","mimeType":"text/plain","callback_url":"http://localhost:9000"}
→ {"registered":"file:///etc/motd","count":1}

GET /v1/mcp/resources
→ {"resources":[{"uri":"file:///etc/motd","name":"motd","description":"message of the day","mimeType":"text/plain"}],"count":1}

# Read it: forwards resources/read to the server that registered it
POST /v1/mcp/resources/read     {"uri":"file:///etc/motd"}
→ the server's result, verbatim · 400 unknown uri · 502 · 504

POST /v1/mcp/resources/deregister     {"uri":"file:///etc/motd"}
→ {"deregistered":true}

# Prompts: the same shape, keyed by name. arguments is the prompt's argument list.
POST /v1/mcp/prompts
{"name":"greet","description":"say hello","arguments":[{"name":"who","required":true}],"callback_url":"http://localhost:9000"}
→ {"registered":"greet","count":1}

GET /v1/mcp/prompts
→ {"prompts":[{"name":"greet","description":"say hello","arguments":[{"name":"who","required":true}]}],"count":1}

POST /v1/mcp/prompts/get     {"name":"greet","arguments":{"who":"agnos"}}
→ the server's result, verbatim · 400 unknown prompt · 502 · 504

POST /v1/mcp/prompts/deregister     {"name":"greet"}
→ {"deregistered":true}
```

A forwarded read or get that the server answers with a JSON-RPC error is answered 502, with the
server's message and code.

## Files (2.5.4)

An index of the files under the operator's roots, with each file's tags and rating, and saved queries
over them. A file's identity is its absolute path.

⛔ **It is off unless the operator names a root**, with `serve --index-root DIR` (repeatable, 16 at most).
The API has no caller authentication until roadmap 2.6.x, so anything local can read what the index
holds, as the user daimon runs as: the names, sizes and times of the files under a root, and which
are identical. Name only what every local process may see. Every route here refuses a request
carrying `Origin`, reads included (403).

The index walks in the background and on restart, and every hour after. Nothing watches the disk, so
`indexed_at` says how stale it is. It keeps to regular files. It follows no symlink, and passes over
hidden directories (`.git`), `/proc`, `/sys`, `/dev`, `/run`, its own data directory, and any name
that is not valid UTF-8, which JSON could not carry. `skipped` counts what it could not take.

### What daimon holds

```
GET /v1/files/index
→ {"state":"fresh","files":41208,"bytes":8123456,"duplicates":12,"skipped":0,"truncated":false,
   "content":true,"terms":8228414,"terms_truncated":false,"indexed_at":1793990381,"generation":3,"persisted":true,"meta_durable":true,"meta_skipped":0,
   "power":"mains","roots":["/home/me"]}
```

`progress` appears while a job runs: `{"phase":"walk","done":251,"total":812}`, the phases being `copy`,
`walk`, `sizes`, `hash`, `group`, `terms`, `read`, `sort` and `save`. During a walk `total` is the last walk's count, and 0 the
first time.

| state | meaning |
|---|---|
| (no connection) | daimon is not running |
| `none` | daimon holds no index. No root is configured, so every route below but this one answers 409 |
| `building` | none is ready yet, and one is coming |
| `catching_up` | one is served, and a newer one is coming |
| `paused` | a job is waiting for mains power (`power` says `battery`). Linux only: on AGNOS `power` is `unknown`, and a walk runs |
| `fresh` | one is served, and nothing is running |

`truncated` means a walk ran out of room (`serve --index-max N`, default 262,144 records, 65,536 on
AGNOS). `persisted` false means the index will not outlive this run, and `meta_durable` false the same
of tags, ratings and saved queries. Those live in a journal, and `meta_skipped` counts its lines that
could not be read.

```
# Re-index a path, or with no body every root. 202: it is queued. A path keeps what lies outside it.
POST /v1/files/index    {"path":"/home/me/photos"}
→ 202 {"ok":true} · 400 · 403 outside the roots · 409 no index
```

### Listing and queries

```
GET /v1/files?untagged=1&under=/home/me&limit=100&offset=0
→ {"files":[{"path":"/home/me/a.txt","size":5,"mtime":1793990000,"rating":0,"tags":["wip"],
             "duplicate_group":3}],
   "total":318,"next_offset":100,"generation":3}
```

`next_offset` is 0 on the last page. A page ends at `limit` (default 100, at most 1000) or about 64 KB
of rows, 8 KB on AGNOS. `generation` changes when a new index is swapped in, which moves rows under a
client that is paging. Rows come in index order, so sort what you fetch. `duplicate_group` is 0 for
none: files sharing one have the same content, by a whole SHA-256 of the whole file. Empty files are
not duplicates of each other.

A query is query-string text, and filters combine. An unknown key answers 400: a misspelled filter must
not answer with every file. Add `q` to ask a question: see [Search](#search-255).

| key | value |
|---|---|
| `tag` | carrying this tag, matched exactly |
| `untagged`, `unrated` | `1`: carrying none |
| `rating` | rated at least this |
| `duplicates` | `1`: sharing their content with another file |
| `under` | at or below this path |
| `ext` | any of these extensions, comma-separated, in any case |
| `q` | a question in words (2.5.5) |
| `kind` | `document`, `image`, `audio`, `video`, `archive`, `code` or `other` |
| `min_size`, `max_size` | at least, or at most, this many bytes |
| `newer_days`, `older_days` | modified within, or before, this many days |
| `limit`, `offset` | paging |

```
# One file: as a row, or {"path","rating","tags","indexed":false} for one a person tagged that the
# index no longer holds. What a person gave is kept against the path.
GET /v1/files/meta?path=/home/me/a.txt
→ a row · 400 · 403 · 404 nothing known
```

### Tags and ratings

A tag is 1 to 64 bytes of UTF-8 with no control byte and no space at either end, and matches exactly.
A file carries at most 32. A request names `path`, or `paths` (up to 256), and all are checked before
anything changes. A path must be spelled one way: absolute, with no empty, `.` or `..` component and
no trailing slash. A tag follows the path, so a rename outside daimon's sight loses it.

```
POST /v1/files/tags     {"paths":["/home/me/a.txt"],"add":["wip"],"remove":["old"]}
→ {"ok":true,"files":1}
GET /v1/files/tags
→ {"tags":[{"tag":"wip","files":3}]}

# What the files at this folder carry, or those beside this file, most used first.
GET /v1/files/tags/suggest?path=/home/me
→ {"suggested":[{"tag":"wip","files":3}]}

# 0 to 5. 0 clears it.
POST /v1/files/rating   {"path":"/home/me/a.txt","rating":5}
→ {"ok":true,"files":1}
```

Each write answers 400 for a bad path or tag, 403 for a path outside the roots, and 409 at a limit:
65,536 files with a tag or rating, 4,096 tags, or a journal at 64 MiB, which a restart compacts.

### Smart folders

A smart folder is a saved query, with a count. Six ship, and saving a query under a name replaces it:

| name | query |
|---|---|
| Recent | `newer_days=7` |
| Duplicates | `duplicates=1` |
| Untagged | `untagged=1` |
| Large & old | `min_size=104857600&older_days=365` |
| Raw only | `ext=arw,cr2,cr3,dng,nef,orf,raf,rw2` |
| Unrated | `unrated=1` |

```
GET /v1/files/smart
→ {"folders":[{"name":"Untagged","query":"untagged=1","files":318,"builtin":true},...]}

POST /v1/files/smart    {"name":"Work","query":"tag=wip&under=/home/me"}
→ 201 {"ok":true} · 400 · 409 at 64 folders

# A page of what it holds, as GET /v1/files. Percent-encode the name.
GET /v1/files/smart/Large%20%26%20old?limit=50

# A folder that ships cannot be deleted (409). Deleting one you redefined puts the shipped query back.
DELETE /v1/files/smart/Work
```

### Search (2.5.5)

`q` is a question in words, and the answer is files, ranked. It is local: no byte leaves daimon.

```
GET /v1/files?q=paid+invoices+last+month&limit=20
→ {"files":[{"path":"/home/me/bills/march-invoice.txt","rank":1,"score":2954,"size":59,"mtime":1793990000,
             "rating":0,"tags":["finance"],"duplicate_group":0,
             "why":"Contains \"paid\" and \"invoices\"; its name or folder has \"invoices\"; modified last month",
             "folders":["Recent","Unrated"]}],
   "total":3,"next_offset":0,"generation":3,"words":["paid","invoices"],"window":"last month",
   "facets":{"kind":[{"label":"document","files":3,"query":"kind=document"}],
             "date":[...],"size":[...],"location":[{"label":"/home/me/bills","files":2,"query":"under=/home/me/bills"}]},
   "duplicates":{"groups":0,"files":0},
   "cost":{"ms":6,"hits":3,"method":"words + name","local":true}}
```

⛔ **It ranks by words, not by meaning, and `cost.method` says so.** `invoices` finds `invoice`, and
`receipt` does not. Ranking by meaning needs a local embedding model, which does not exist yet.

**What is searched** depends on the operator. With `--index-root`, the file's name and its folder's:
`method` is `name`. With `serve --index-content` too, the first 64 KB of each file with no NUL in its
first 4 KB: `method` is `words + name`. That is a second opt-in, because a local process can then ask
whether a file it could not open holds a word. So a PDF is found by name alone. Status says which
(`content`), and `terms_truncated` if the reserve for words ran out.

A file with more of the question's words ranks first, and a name match counts for more than saying a
word often. Words are folded and lightly stemmed (`invoicing` meets `invoice`, in English), and at most
8 count. Words like `the` and `find` do not. `why` is built from what matched, and infers nothing.
`today`, `yesterday`, and `this` or `last` with `week`, `month` or `year` are windows, rolling rather than
by the calendar (`last month` is 30 to 60 days ago), and every other filter works beside a question.

Ranking stops at 1,200 results. Add a facet's `query` to your own to refine by it: each gives the count it
shows. A question can be saved: `POST /v1/files/smart {"name":"Money","query":"q=paid+invoices"}`.

The rate limit below applies, so poll `GET /v1/files/smart` and `/v1/files/index` a few times a minute,
not per frame. Counts are kept until something they read changes. The index, tags and queries all
survive a restart, in `<data_dir>/files` (`serve --data-dir DIR`, default `/var/lib/agnos`). See ADR-010.

## RAG Pipeline

```
# Ingest text: chunked, embedded and indexed (a longer text gives more chunk ids)
POST /v1/rag/ingest
{"text":"Rust is a systems programming language","metadata":"source1"}
→ 201 {"chunk_ids":[1]}

# Query: the best-matching chunks, formatted as context for a model
POST /v1/rag/query
{"query":"rust safety"}
→ {"formatted_context":"Use the following context to answer the question.\n\n---\n[1] Rust is a systems programming language\n\n---\n\nQuestion: rust safety"}
```

The index is in memory, so it starts empty each time daimon starts. `vector_entries` in
`/v1/metrics` counts it.

## Edge Fleet

```
# Register node — capabilities optional (2.3.4); defaults x86_64 / 4 / 4096 / 32768 / false
POST /v1/edge/nodes
{"name":"edge-1","arch":"aarch64","cpu_cores":4,"memory_mb":8192,"disk_mb":65536,"has_gpu":true}
→ 201 {"id":"1"} · 400 name taken · 422 an invalid capability

# List nodes (optional ?status=online|suspect|offline|updating|decommissioned; any other value lists all).
# Here status is a number: 0 Online, 1 Suspect, 2 Offline, 3 Updating, 4 Decommissioned.
GET /v1/edge/nodes
→ {"nodes":[{"id":"1","name":"edge-1","status":0}]}

# Get node
GET /v1/edge/nodes/1
→ {"id":"1","name":"edge-1","status":"Online","active_tasks":0,"arch":"aarch64","cpu_cores":4,
   "memory_mb":8192,"disk_mb":65536,"has_gpu":true}

# Heartbeat
POST /v1/edge/nodes/1/heartbeat
{"active_tasks":"3","tasks_completed":"10"}
→ {"ok":true}

# Decommission
POST /v1/edge/nodes/1/decommission
→ {"ok":true}

# Fleet stats
GET /v1/edge/stats
→ {"total":1,"online":1,"suspect":0,"offline":0,"updating":0,"decommissioned":0,"active_tasks":0,
   "tasks_completed":0,"cpu_cores":4,"memory_mb":8192,"gpu_nodes":1}
```

A node's health is worked out whenever the fleet is read. A node with no heartbeat for 30 s is
Suspect, and after 90 s it is Offline.

Edge node ids have their own sequence since 2.3.4. Before, they came from the agent counter, so
after five agents the first node was `"6"`. `cpu_cores`, `memory_mb` and `gpu_nodes` in the stats
total the nodes that can take work (not offline, not decommissioned). `arch` is 1–32 characters of
`a-z`, `0-9` and `_`. `cpu_cores` is 1–65536, and `memory_mb` and `disk_mb` are whole numbers.

## Scheduler

```
# Register compute node
POST /v1/scheduler/nodes
{"node_id":"worker-1","total_cpu":"8","total_memory_mb":"16384"}
→ 201 {"ok":true}

# Submit task. The id is samay's, a UUID.
POST /v1/scheduler/tasks
{"name":"train-model","agent_id":"agent-1","priority":"7"}
→ 201 {"task_id":"21cecebe-6079-4fe7-9618-e533df976a7d"}

# Task counts. This route lists no tasks: find a node's with its work list, below.
GET /v1/scheduler/tasks
→ {"stats":{"total_tasks":1,"queued":1,"running":0,"completed":0,"failed":0}}

# Get task — agent_id, node (where it was placed) and fail_reason since 2.3.1
GET /v1/scheduler/tasks/21cecebe-6079-4fe7-9618-e533df976a7d
→ {"task_id":"21cecebe-6079-4fe7-9618-e533df976a7d","name":"train-model","priority":7,"status":"Queued","agent_id":"agent-1","node":null,"fail_reason":null}

# Cancel task
POST /v1/scheduler/tasks/{id}/cancel
→ {"ok":true} · 404 unknown, or its status does not allow a cancel

# Schedule pending tasks: best fit over the registered nodes
POST /v1/scheduler/schedule
→ {"decisions":[{"task_id":"21cecebe-…","assigned_node":"worker-1","reason":"best-fit selection","score":1.0}]}

# An executor's work list: the node's Scheduled and Running tasks (2.3.1)
GET /v1/scheduler/nodes/worker-1/tasks
→ {"node_id":"worker-1","tasks":[{"task_id":"21cecebe-…","name":"train-model","priority":7,"status":"Scheduled","agent_id":"agent-1","node":"worker-1","fail_reason":null}],"count":1}
→ 404 unknown node

# Start — the executor reports it running: Scheduled → Running (2.3.1)
POST /v1/scheduler/tasks/{id}/start
→ {"task_id":"21cecebe-…",...,"status":"Running","node":"worker-1",...}
→ 404 · 409 unless Scheduled · 403 from a browser

# Complete — and done: Running → Completed, or Failed with a reason (2.3.1)
POST /v1/scheduler/tasks/{id}/complete                                  (no body = completed)
POST /v1/scheduler/tasks/{id}/complete {"status":"failed","reason":"disk full"}
→ {"task_id":"21cecebe-…",...,"status":"Failed",...,"fail_reason":"disk full"}
→ 400 status not completed/failed · 404 · 409 unless Running · 403 from a browser

# Scheduler stats — the averages are 0 until a task has started (2.3.1)
GET /v1/scheduler/stats
→ {"total_tasks":1,"queued":0,"running":0,"completed":0,"failed":1,"average_wait_time_ms":15,"average_run_time_ms":0}
```

A task's life through the API: submit → schedule places it on a node (Scheduled, holding that node's
capacity) → the executor on that node finds it with `GET /v1/scheduler/nodes/{id}/tasks`, reports it
started (Running), then reports it complete (Completed or Failed). Completing it returns the
capacity at once, so the next schedule can use it. `status` for complete accepts `completed` /
`failed` or `Completed` / `Failed`.

Anyone who can reach the API can report any task. Only browsers are refused (the Origin rule, as for
agents), and executor identity waits for authentication (roadmap 2.6.x).

## Metrics

```
GET /v1/metrics
→ {"agents":1,"mcp_tools":13,"mcp_resources":0,"mcp_prompts":0,"vector_entries":5,"edge_nodes":3,
   "federation_nodes":0,"ipc_messages":42,"ipc_refused":1,"bus_dropped":0,"ipc_channels":1,
   "bus_bytes":2048,"http_evicted":0}
```

`federation_nodes` is always 0 for now: no route adds a federation node.

Since 2.3.3:
- `ipc_messages` counts agent messages put on the bus;
- `ipc_refused` counts frames answered NACK, plus channels closed on a fault;
- `bus_dropped` counts messages not queued because a queue (100 per agent) or the bus (64 MiB) was
  full.

Since 2.3.4:
- `ipc_channels` is the number of open agent channels;
- `bus_bytes` is the bytes of messages the bus holds (freed as they are taken).

Since 2.4.2, `http_evicted` counts connections closed to make room while every slot was taken.

See [agent-ipc.md](agent-ipc.md).

## Error Responses

| Status | Meaning |
|---|---|
| 400 | Bad Request — missing/invalid field, or a body that is not one JSON object (see Request bodies) |
| 403 | Forbidden — a host name other than loopback's while daimon listens on loopback; a state change from another site; agent or task control from any browser |
| 404 | Not Found — unknown route or ID |
| 405 | Method Not Allowed — a route exists, not for this method |
| 409 | Conflict — the agent's status does not allow the action, a name is taken, the agent limit is reached, or output is not captured |
| 413 | Payload Too Large — body > 64 KB |
| 422 | Unprocessable Entity — validation failure |
| 429 | Too Many Requests — rate limit (120/min per IP), or an agent's message queue is full |
| 500 | Internal Server Error — e.g. an agent's rlimits could not be applied, or its executable could not be run |
| 501 | Not Implemented — chunked Transfer-Encoding; pause and resume on AGNOS |
| 502 | Bad Gateway — an MCP endpoint could not be reached or answered wrongly |
| 503 | Service Unavailable — on AGNOS: no free channel for another agent, or 4 calls to other servers still running |
| 504 | Gateway Timeout — an MCP call, `web_fetch` or `web_search` did not finish within 60 s |

A handler's error answers `{"error":"message","code":NNN}`. Four answers are shorter:
- an unknown route: `{"error":"not found"}`;
- a wrong method: `{"error":"method not allowed"}`;
- the rate limit: `{"error":"too many requests"}`;
- a request refused before it is read (400 for an ambiguous length, 413, 501 for chunked): no body.

## Rate Limiting

120 requests a minute from each source IP, counted in a fixed 60 s window that starts with its first
request. Past that, 429. A client whose address daimon cannot learn draws on one shared bucket of 960
a minute. That is every client on AGNOS, which gives no peer address.

## Security

- There is **no authentication** (roadmap 2.6.x). The defaults limit who can reach the API. It binds
  127.0.0.1, answers only to loopback host names while it does, and refuses state changes from
  other sites on every route. Agent and task control refuse any browser-originated request.
- All user-controlled strings in responses are JSON-escaped.
- Content-Length is validated; Transfer-Encoding is rejected.
- Maximum request size: 64 KB.
- An agent's executable is chosen by daimon from its type, never by a request. See Agents.
- Request bodies are parsed strictly (see Request bodies). Every string field is decoded, and a
  body with a duplicate key (at any depth since 2.5.1) or U+0000 in a top-level string is refused.
