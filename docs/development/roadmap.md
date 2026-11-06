# Daimon Roadmap

> **Scope**: open work only. Shipped work is recorded in [CHANGELOG.md](../../CHANGELOG.md), not here — if an item's text is mostly explaining what was *fixed*, it belongs there instead.
>
> **Severity legend**: **P0** blocking (security / correctness — must-fix before ship) · **P1** high (must-have for the current arc) · **P2** medium (schedule when capacity opens) · **P3 / Low** nice-to-have, no urgency. Upstream-blocker items quote the upstream tracker's own severity.

**Where daimon stands** — `2.5.4`, cyrius 6.7.18, every dependency at its latest release (bote 3.8.15,
nein 1.9.1). It builds for x86_64, aarch64 and AGNOS, and CI runs the suites, the HTTP smoke and the
fuzz harnesses on Linux, every suite and the binary's smoke on a real aarch64 kernel, and the AGNOS
guest test on agnos 1.57.17.
- Agents are registered, started under their limits, stopped, paused, resumed and deleted through
  the API, each in a cgroup of its own where daimon can make one. They talk to daimon on a channel.
- Scheduled tasks are placed by samay, started and completed through the API.
- daimon runs one thread and its own event loop. A call that waits on another server runs in a child.
- The API binds 127.0.0.1 unless `--listen` says otherwise, refuses other sites' pages, and has no
  caller authentication. Everything that changes the firewall, reads its live state, or archives
  the audit chain is refused until it does (2.6.x).
- Each MCP tool's first definition is pinned across restarts, and a change is marked and audited.
- With `--index-root`, daimon holds an index of the files there, with tags, ratings and smart folders.

## The releases ahead

Each line is a release train, not a single release. One bite at a time, suite green at every step.

| release | theme | severity | why this order |
|---|---|---|---|
| **2.6.x** | **Caller identity and MCP authentication** | P1 | Every gated tool, the memory API and task reporting wait on it, and 3.0.0 needs it to know what a tenant is. |
| **2.7.x** | **Adopting agnos 1.57.7** — limits, pause and resume, captured output, one write per answer | P2 | agnos closed every gap daimon filed; daimon still runs its interims. Independent of 2.6.x. |
| **2.8.x** | **Serving hardening** — the leads 2.4.4 – 2.5.2 left, bote's consumer audit | P2 | Bounded, measurable items; none blocks another. |
| **2.9.x** | **Modules without a route** — federation, screen capture, the memory API | P3 | The memory API needs 2.6.x. |
| **3.0.0** | **Per-agent arena isolation** (VULN-007's open half) | P0 when any gate below opens | Changes the allocation model under every agent. Needs 2.6.x. |

⚠ The 3.0.0 line is where the security gates open, not where they are first considered. Each `2.x.0`
cut re-evaluates the P0 below.

---

## 2.6.x · P1 — Caller identity and MCP authentication

`POST /v1/mcp/call` and every other route have **no caller authentication**: a request is answered
for whoever can reach the port. What exists stops browsers, not a local process:
- every state change is POST or DELETE, and one from another site's page is refused on every route;
- the API binds 127.0.0.1 by default, on AGNOS too since 2.4.4, and while it does a request must
  name a loopback host (DNS rebinding);
- agent and task control answer 403 to any request that carries `Origin`.

**Still open**: anything on the host can do everything, and so can any client on the network once
`--listen` opens it. bote's `claims` argument, the seam an identity arrives through, is a reserved
`0` in its 3.x ABI.

Refused until this lands, each with an operator flag that reopens it on a trusted single-tenant host:
- `nein_allow` / `nein_deny` (change the firewall; `_nein_admin_enabled`, since 2.1.8);
- `nein_status` / `nein_list` / `nein_diff` (run `nft` as root, return the live ruleset;
  `_nein_live_reads_enabled`, since 2.5.2);
- `libro_retention` (archives the audit chain; `_libro_retention_enabled`, since 2.5.1);
- accepting a changed tool definition (`POST /v1/mcp/tools/{name}/accept`; `serve --mcp-pin-accept`,
  since 2.5.4).

Also waiting on identity:
- **The agent memory API.** The store works, but no route reaches it, and one must not until a caller
  can be tied to an agent: otherwise any client reads any agent's memory. Its records also need the
  Rust original's `tags` field: `list_by_tag` is a substring search over the record today.
- **Who may replace an external MCP registration.** Re-registering a name replaces what is served,
  whoever registered it first. (Taking a builtin's name is refused since 2.2.3. Since 2.5.4 it cannot
  move the pin, and is marked and audited.)
- **Who may accept a tool definition, per consumer, through t-ron.** What is left of thoth's filing:
  2.5.4's accept is one decision for the host. It also closes trust on first use's gap: identity can
  say who made a first registration.
- **Who may see the file index.** Any local process reads what an operator's roots hold, and edits
  tags, ratings and saved queries. Decide per caller, and whether a caller sees only what its own
  permissions would show.
- **Who may report a task** (VULN-016). Any caller can mark any task running, completed or failed,
  and completing one early returns its capacity while the work may still run.
- **`libro_export` / `libro_query`** answer the whole audit chain to any caller (bote's consumer
  audit, Medium). Read-only, so not gated; decide what a caller may see once callers exist.

## 2.7.x · P2 — Adopting agnos 1.57.7

agnos 1.57.6 and 1.57.7 closed all twelve filings daimon made (`docs/development/issues/archived/
2026-09-23-*.md` in the agnos repo). daimon has taken the loopback listener, a stop that ends the
agent, the wait status (2.4.4), argv as separate arguments and the spawn failure's reason (2.5.2).
Still to adopt, each verified in the guest test:

| agnos filing | daimon today | to do |
|---|---|---|
| `no-per-process-resource-limits` | agents run without limits, audited `agent.limits.unenforced` | arm `spawn_limits`#107 (pages, ms) before the spawn; `limits_enforced:true`; refuse a start whose arm fails; a CPU-burning fixture to see SIGXCPU (wait status 280) |
| `parent-cannot-end-stop-or-continue-a-child` | pause / resume answer 501 | signals 19 / 18; the guest test reads `PROCLIST_STOPPED`; reach an agent's descendants |
| `child-inherits-every-fd-and-spawn-arms-leak` | `--agent-output capture` refused at startup | `SPAWN_F_CLEANFD` with the redirect armed; capture stdout and stderr |
| `sock-send-and-connect-hold-the-cpu` (P1 for daimon) | answers written 512 bytes at a time, 1 ms apart | measure a whole-answer write on 1.57.14 under a CPU quota; one write per answer if it holds |
| `tcp-server-cannot-be-loopback-only` | loopback-only listener (2.4.4), one rate-limit bucket for every client | per-client rate limiting, if agnos gives a peer address |
| `no-way-to-read-the-power-source` (filed 2026-11-06, **open** in agnos) | `power` is `unknown`, and a file walk runs on battery | an agnos arm in `_fx_power_find`, once a source exists |
| `inbound-tcp-syn-dropped-by-isr-drain`, `socket-ids-have-no-owner` | nothing in daimon | re-measure accept from the host; close audit AG-1 and AG-8 |

Nothing to change in daimon for `sleep-ms-holds-the-cpu`, `sock-recv-never-reports-eof-after-peer-fin`
and `tsc-calibration-refused-stops-the-us-clock` (`daimon_now_ms` stays for older kernels).

## 2.8.x · P2 — Serving hardening

- [ ] **A whole-answer deadline for writes on Linux** (2.4.4). An answer goes through sandhi's
  `sock_send_all` or daimon's `_srv_send_all`, both bounded per write by `SO_SNDTIMEO`, so a local
  client that reads a trickle of an answer larger than its socket buffer holds the loop while each
  write makes progress. Not measured.
- [ ] **External MCP callback URLs** (bote's consumer audit, Medium). A registration's `callback_url`
  is checked for its scheme only, so daimon forwards calls wherever a caller points it, loopback
  and private addresses included. Local MCP servers on loopback are the main use, so this needs a
  policy (an allow-list, or the operator's), not bote's `web_fetch` guard as it is.
- [ ] **The relayed-error envelope** (bote's consumer audit, Medium): what an external server's error
  answer may put into daimon's reply.
- [ ] **The agent-frame arena** (2.5.1 lead, not measured). `_ipc_arena` is the stdlib's growable
  arena, which bote found keeps a new chunk for each allocation larger than any before (filed with
  cyrius by bote, 2026-11-02). Frames are capped at 64 KB and the arena is rewound per frame, so
  growth should stop at a high-water mark: measure it with frames of rising size.
- [ ] **The request-body parse cost** (2.5.1). bayan 1.11's bounds-checked parser made
  `http_body_read3` 47% slower (1.30 → 1.91 µs). Upstream's; re-measure at each bayan move.
- [ ] **nein's live readers on the loop.** If an operator sets `_nein_live_reads_enabled`, each call
  waits on `nft` on the loop, up to 60 s since nein 1.8.2. Detach them (`mcp_builtin_waits`), with
  the gate's audit entry made before the child is forked.
- [ ] **A digest through a swapped directory** (2.5.4 audit R-3). `O_NOFOLLOW` covers a file's final
  component, so a directory replaced by a link after the walk can have a file outside a root read for
  its digest. `openat2(RESOLVE_NO_SYMLINKS)` closes it on Linux; agnos would need a peer.
- [ ] P3: CI resolves with `cyrius deps --no-lock` then `--verify`; `cyrius deps --locked` (cyrius
  6.7.6) checks the resolve against the lock in one step. Rehearse it in CI's own steps first.

## 2.9.x · P3 — Modules without a route

- [ ] Routes for the federation and screen-capture modules. Both came with the port and have their
  own tests, but no route reaches them, so `federation_nodes` in `/v1/metrics` is always 0.
- [ ] The agent memory API, once 2.6.x can tie a caller to an agent.

## 3.0.0 · P0 (gated, dormant) — VULN-007: per-agent arena isolation

**MUST be resolved before enabling any of**: multi-tenant hosting, kavach sandboxing, untrusted
federation, or external MCP callbacks (bote). Dormant today because no consumer has flipped any of
those; **P0 the moment one does.** Re-evaluate at every `2.x.0` cut.

Zero-on-reset and secret hygiene close the *reuse* and *leak* channels but do not **isolate trust
domains**: one bump allocator still backs every agent. Isolation is the open half and the hard
prerequisite. Needs 2.6.x to know what a tenant is.

## Beyond (unsequenced)

Severity assigned when one is scheduled.

- [ ] **Ranked file search** — crab's M8 (item 5 of its filing, the half 2.5.4 left): a question in words
  answered with ranked files and why each matched. It needs a local embedding that can rank by meaning,
  which RAG's 32-slot hash cannot, and should share the file index with mneme, whose README names
  `/v1/knowledge/*`, which no route serves.
- [ ] jnana integration — grounded knowledge queries backed by verified AGNOS science data
- [ ] gRPC transport option alongside HTTP
- [ ] WebSocket streaming for real-time agent events
- [ ] Agent migration between nodes
