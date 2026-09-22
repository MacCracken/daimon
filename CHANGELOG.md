# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [2.2.2] - 2026-09-22

**Two state-changing endpoints could be triggered by a plain GET — and so by any web page a local
operator opened.** Fixed at the router and at its root cause. Plus **samay 1.1.3**, which moves the
2.2.1 string-ownership fix into the library where it belongs.

**368 tests** (was 356) across nine files, 5 fuzz harnesses, `fmt` / `lint` / `vet` clean, all three
targets build. cyrius 6.6.6.

### Security — CSRF by GET: decommission and cancel accepted any method

`/v1/edge/nodes/{id}/decommission` and `/v1/scheduler/tasks/{id}/cancel` dispatch by path suffix
and checked no method. A plain **GET** performed the state change. Measured on the live binary:

```
GET /v1/edge/nodes/1/decommission     -> 200  node status 0 -> 4  (decommissioned)
GET /v1/scheduler/tasks/<id>/cancel   -> 200  Queued -> Cancelled
```

GET is the one method a browser issues freely: a cross-origin `<img src=…>`, a link prefetch or a
crawler needs **no CORS preflight**, and daimon has no authentication. So any page a local operator
visited could reach a daimon on `localhost:8090`. Edge node ids are sequential integers, which lets
`<img src="http://localhost:8090/v1/edge/nodes/1/decommission">` walk the entire fleet. Task ids
are UUIDs, which limits the cancel half to an attacker who has seen one.

Both now require **POST** and answer **405** to anything else. After the fix, same probe:

```
GET  …/1/decommission -> 405   status stays 0      POST -> 200   status 4
GET  …/<id>/cancel    -> 405   stays Queued        POST -> 200   Cancelled
```

### Fixed — the root cause: an unrecognised HTTP verb was treated as GET

`http_parse_method` ended `return 0;`, so `OPTIONS`, `PATCH` or any garbage token parsed as **GET**.
The router's 2.1.0 MCP section had documented this — *"http_parse_method maps ANY unrecognized verb
to 0 (GET), so a route must never be written as 'if not POST then GET'"* — and guarded **only its
own routes** against it. The source was never fixed and no other route was guarded: the same
fix-one-site-never-sweep pattern as the aliasing class in 2.2.1.

An unknown verb now returns **-1**, which matches no route. `HEAD` maps to GET explicitly — it is
GET without a body by definition, health-checkers send it, and as a safe method it can never reach
a state-changing route. Pinned by `tests/http.tcyr` against the real module, on crafted request
buffers; reverting to `return 0` fails 4 of its 12 assertions.

### Fixed — every route now states its method (27 arms)

A sweep of `src/router.cyr` found **14 routes with no method check**. Beyond the two above, the
state-changing ones were `/v1/mcp/call` (executes a tool), `/v1/rag/ingest`,
`/v1/edge/nodes/{id}/heartbeat`, `/v1/scheduler/nodes` and `/v1/scheduler/schedule`; the rest were
read-only routes that accepted `POST`/`DELETE` as if they were `GET`. All 16 single-method routes now
go through a `route_method` guard. The 11 multi-method routes that *did* check fell through on a
mismatch to the end of the router and answered **404** — "no such resource" for a resource that
exists — and now answer **405**. Verified with a live matrix: every correct method still returns
200, including `HEAD /v1/health`; every wrong method and every unknown verb returns 405.

⚠ **Behaviour change for a non-conforming client.** A client that reached a mutation with the
wrong verb — a `GET` to `/cancel`, say — now gets 405. That was the vulnerability; a client using
the documented method is unaffected.

This also resolves the 2.2.1 note that `/v1/scheduler/nodes` answered a GET with a misleading
`400 missing body`: it is now an honest 405. Listing nodes is still not supported — that is a
missing feature, not a routing bug, and is not added here.

### Fixed — `500` went out as `500 Error`

`_http_reason` had no entry for 500 and fell through to the literal `"Error"`. `error_to_status`
maps every non-4xx `DaimonError` to 500, so this was the common error path. It now sends the RFC
7231 `Internal Server Error`, and `405 Method Not Allowed` is added alongside.

### Changed — samay 1.1.2 → **1.1.3**

samay now owns every Str it retains — **20 fields across 8 constructors**, found by sweeping the
library rather than fixing only the two sites daimon exposed. 2.2.1 had to clone task and node
strings at daimon's own boundary as a stopgap; those clones are **removed**, and the live leak stays
closed with them gone:

```
POST /v1/scheduler/tasks {"name":"ORIGINAL_TASK_NAME"}  + one unrelated request
GET  /v1/scheduler/tasks/<id>  ->  "name":"ORIGINAL_TASK_NAME"     (daimon clones removed)
```

which is the proof that the fix now lives in the library, covering every consumer. samay's own
changelog has the detail, including the measured cost (+179 ns on `scheduled_task_new`).

### Performance

The 19 mirrored benchmarks cannot observe router or `src/` changes (2.2.1 showed their binary is
byte-identical across `src/` edits). The one path that changed cost is samay's constructor, measured
in samay: **+179 ns (+8.8%)** per task submission, isolated from the toolchain by A/B'ing the same
tree with ownership as a pass-through.

## [2.2.1] - 2026-09-22

**Three live cross-request data leaks fixed — found by finishing a sweep that 2.1.6 asked for and
nobody did.** Plus the test-integrity arc's second batch: `error`, `supervisor` and `ipc` are off
their mirrors, which surfaced three more latent defects of the same class and two of other kinds.

**356 tests** (was 306) across eight files, 5 fuzz harnesses, `fmt` / `lint` / `vet` clean, all three
targets build. cyrius 6.6.6 and all nine pins unchanged.

### Security — three LIVE cross-request data leaks on unauthenticated endpoints

The 2.1.6 RAG filing ended with *"Sweep, don't spot-fix. Audit every `jget` result that outlives its
handler across `src/api_*`."* That sweep never happened. Doing it found the same bug on three more
**live, reachable** endpoints. Each stored a `jget` **view** into the per-connection request buffer —
which sandhi **reuses** — so the next request from *any* client overwrote the stored field, and a
later read served that client's bytes back. Each was probed on the real binary before and after:

| endpoint | registered | after one unrelated request, served back |
|---|---|---|
| `POST`/`GET /v1/agents` | `"name":"ORIGINAL_AGENT_NAME"` | `"name":"y\":\"ZZZZZZZZZZZZZZZ"` |
| `POST`/`GET /v1/edge/nodes` | `"name":"ORIGINAL_EDGE_NODE"` | `"name":"ZZZZZZZZZZZZZZZZZZ"` |
| `POST /v1/scheduler/tasks` → `GET …/{id}` | `"name":"ORIGINAL_TASK_NAME"` | `"name":"ZZZZZZZZZZZZZZZZZZ"` |

All three now read back byte-for-byte. Fixed at the **retention** boundary, per the RAG precedent:
`agent_handle_new` and `edge_node_new` own the Strs they store, so every future caller is covered.
The scheduler is different: tasks are stored by **samay**, whose `scheduled_task_new` and
`node_capacity_new` store their Strs raw. daimon closes it at *its* boundary (`api_sched_submit`,
`api_sched_register_node`) because `lib/samay.cyr` is vendored and regenerated; the root cause is
upstream and recorded below. Regression tests use the 1.2.5 clobber shape and are
**mutation-proven** — each fails against the unfixed code.

**Verified clean, not assumed:** MCP tool, resource and prompt registration (the 1.2.5 fix covers
all three), RAG ingest (2.1.6), and every `str_cstr(...)` map key in `mcp.cyr`, `edge.cyr` and
`fed_vector_store.cyr` — `str_cstr` allocates and copies.

### Fixed — the same class, latent (no live caller yet), found by the test migration

- **`supervisor_register`** keyed three maps by `str_data(agent_id_str)`. `map_new()` keys are
  **borrowed** — `map_set_a` does `store64(ep, key)`, it never copies — so the key pointed into the
  caller's buffer. 2.3.x will call this with an id from `/v1/agents/{id}`.
- **`msg_bus_subscribe`, `msg_bus_register_name`, `rpc_register_method`** stored caller cstrs as
  keys *and* values. Against the unfixed code the clobbered lookup returned 0 and the next `streq`
  on it **SIGSEGV'd**. All five sites now go through one `_ipc_own` helper.

### Fixed — `rpc_unregister_agent` compared ids by pointer

`if (handler == agent_id_cstr)` is pointer equality. It removed a method only if the caller passed
the *identical pointer* used at registration, so an unregister request carrying the same id in a
different buffer removed nothing and the departed agent's methods stayed routable. Now `streq`. The
two fixes had to land together: once registration owns its values, no caller pointer can ever equal
a stored one.

### Fixed — `error_json` inserted `msg` unescaped

A `"` or `\` in the message broke the JSON, and a message shaped `x","admin":true,"y":"` injected
fields. **Latent** — all ~20 callers pass constants — but the natural next line anyone writes,
`http_bad_request(fd, str_cat("unknown key: ", key))`, would have made it live. `json_escape_str`
moved from `src/http.cyr` into `src/error.cyr` (included earlier, so every caller still resolves it)
and `error_json` now uses it. Mutation-proven: the raw insert fails all three injection assertions.

### Changed — test integrity, batch two (roadmap 2.2.x)

| module | mirror covered | real-module tests | what the migration found |
|---|---|---|---|
| `error` | enum + 3 of 4 fns | in `daimon.tcyr` | the mirrored enum had **drifted**: it defined `DAIMON_ERR_IO = 10`, a code `src/` never has, while the real code 10 (`DAIMON_ERR_IPC_FAULT`) had never been tested; `error_json` had never run |
| `supervisor` | 11 of 26 | `tests/supervisor.tcyr` (43) | the registry aliasing above |
| `ipc` | 10 of 28 | `tests/ipc.tcyr` (24) | aliasing ×3, pointer-compare unregister |
| `fuzz/circuit_breaker` | 4 fns, magic numbers | real `supervisor.cyr` | asserted `load64(cb) != 1` for OPEN — correct only while `CircuitState` stays 0/1/2; now names the enum |

Every assertion the mirrors carried was moved forward, not dropped. `tests/edge.tcyr` is new (the
edge regression). `tests/daimon.tcyr` shrinks by ~300 lines of mirror.

### Note — on "bare errors"

Asked to clean up daimon's remaining bare `ERR_*`: **there are none.** The enum has been
`DAIMON_ERR_*` since 1.4.2, `cyrius lint` reports zero `lint_error_enum_namespace` notes across
`src/`, `tests/` and `fuzz/`, and the build has zero `ERR_*` duplicate-symbol collisions. The only
two `ERR_*` tokens in the tree are in a historical comment. The error-module work above — the
drifted mirror, the untested code, the unescaped `error_json` — is what that request turned up
instead.

### Recorded — found, not fixed here

- **samay stores its Strs raw.** `scheduled_task_new` (name, description, agent_id) and
  `node_capacity_new` (node_id) retain caller Strs without copying. daimon defends at its boundary;
  the library should own what it retains. Upstream item for samay.
- **`/v1/scheduler/nodes` has no method check.** Every method routes to the register handler, so
  nodes cannot be listed and a `GET` answers `400 missing body`. Also why the node-id fix could be
  verified only by code, not by a live read-back.
- **Latent aliasing with no live route**: `federation.cyr:62`, `fed_vector_store.cyr` (collection
  and dedup keys), `screen.cyr:69,99`. Nothing reaches them from an entry point today; they come in
  scope with the arcs that wire them.

### Performance

The 19-benchmark suite is **byte-identical** before and after this release: `tests/daimon.bcyr` built
from the 2.2.0 tag and from this tree `cmp` equal. So its results are *provably* unchanged — and
that is also the clearest statement of the test-integrity problem: the benchmark suite cannot observe
a single one of this release's seven fixes, because it mirrors `src/` rather than including it.

## [2.2.0] - 2026-09-22

**The test-integrity arc opens, and its first bite found a bug that had been live since the port.**
`src/agent.cyr` is now tested against itself rather than a mirror — and the first time that code
ever ran, `read_vm_rss` turned out to return **0 for every process, always**.

**306 tests** (was 293): +33 real agent assertions, −20 that tested a local reimplementation. 5 fuzz
harnesses, `fmt` / `lint` / `vet` clean, all three targets build. Also picks up **nein 1.7.0**.

### Fixed — `read_vm_rss` could never parse a number

```cyrius
while (j < n) {
    if (load8(buf + j) != 32) { break; }   # not a space -> break
    if (load8(buf + j) != 9)  { break; }   # not a tab   -> break
    j = j + 1;
}
```

A byte cannot be both a space and a tab, so whichever it is, one of the two tests fires on the
**first iteration** and `j` never advances. `/proc/<pid>/status` writes `VmRSS:\t   12345 kB` — a
**tab** — so `j` stayed on the tab, the digit loop below it saw `9` (< 48) and stopped immediately,
and the function returned `0 * 1024` for every process it was ever asked about.

The break now requires **both** to be false. Found the first time `read_vm_rss` was executed: it has
no caller in `src/` (the supervisor is roadmap 2.3.x) and the old mirror-based test only ever passed
it `0` and a bogus pid — both of which short-circuit before reaching the parse.

### Added — `tests/agent.tcyr`, against the real module

33 assertions over `src/agent.cyr` **and its real dependency chain** (`src/syscalls.cyr`,
`src/error.cyr`, `src/audit.cyr` → libro). The 20 assertions it replaces in `tests/daimon.tcyr` ran
against local copies covering 10 of the module's 28 functions and none of the `/proc` readers.

⚠ **`agent_next_id` stays mirrored in `daimon.tcyr`** — the screen, scheduler and mcp mirrors still
call it, and untangling those is their own bite. Recorded rather than glossed: this arc is
explicitly one module at a time, suite green at each step.

### Note — a second finding, pinned rather than fixed

`dir_list` returns an **empty vec** for `/proc/<pid>/fd` while enumerating an ordinary directory
fine (the test asserts `/tmp` as a control). So `count_fds` and `count_threads` — both written
correctly against `dir_list`'s contract — are structurally `0` and `1`, and `agent_update_resources`
stores those. Once the supervisor is wired, `/v1/agents` would serve `fds_used: 0` forever.

The defect is **below daimon**: procfs returns entries the stdlib's `getdents` walk does not
surface. daimon's code needs no change, so the test asserts the behaviour **as it actually is**,
labelled `KNOWN GAP` — which keeps the suite honest *and* makes the assertion fail the day the
stdlib fixes it, pointing at the line to flip back.

### Changed — nein 1.6.12 → 1.7.0

nein's error enum is now fully `NEIN_ERR_*` namespaced. Its four remaining bare `ERR_*` members were
the last `lint_error_enum_namespace` findings in that tree, and one — `ERR_CHAIN_NOT_FOUND` (= 4) —
collides *by value* with bote's bare `ERR_PARSE` (= 4) in any host linking both, which daimon now
is. **Values are unchanged**, so `nein_err_code()`'s contract holds and daimon, which names none of
them, needed no change. Source-breaking for nein's own consumers, hence its minor bump.

## [2.1.8] - 2026-09-22

**Roadmap restructured around the arc to 3.0.0, two false "upstream blockers" retracted, and the
nein firewall integration attempted, measured and deferred.** No functional change to daimon: all
three targets build (x86_64 · aarch64 · agnos), **293 tests**, 5 fuzz harnesses, `fmt` / `lint` /
`vet` clean, 21 benchmarks unchanged. cyrius stays at 6.6.6; dep pins unchanged.

### Changed — the roadmap is forward-facing and sequenced

It carried shipped work as content (the VULN-007 entry led with two closed halves; the AGNOS entry
led with "builds and boots as of 2.1.7") and stale facts ("235 tests", "Future v1.4.0+" on a 2.x
project). Rewritten: every section is open work, every number verified against the tree, and the
items are now **release trains toward 3.0.0** — 2.2.x test integrity · 2.3.x agent lifecycle ·
2.4.x AGNOS spawn+IPC · 2.5.x agent identity + MCP auth · 3.0.0 per-agent arena isolation, each
line stating why it must follow the one above.

⭐ **The largest gap was not on the roadmap at all**: daimon is the AGNOS agent orchestrator and
**cannot start an agent**. `agent_spawn_with_limits`, `agent_start`, `agent_stop`, `agent_ipc_bind`,
`ipc_send` and `msg_bus_publish` have **zero callers**; `src/router.cyr` exposes list / register /
get and no lifecycle route. Now the 2.3.x line.

### Fixed — two "blocked on upstream" items were false and are retracted

Both were stale daimon-side assumptions. Filing either upstream would have put a wrong issue in
front of a maintainer.

- **nein** — the roadmap said nein "has no `mcp.cyr`; the Rust `mcp.rs` is unported". nein has
  shipped `src/lib/mcp.cyr`, `dist/nein-mcp.cyr` and all six tools since **1.6.0**, where that
  surface was *deliberately redesigned* into flat-arg tools. It even ships a guide with a section
  written for daimon by name. Nothing was blocked.
- **sandhi** — the roadmap said `max_conns` was accepted-but-not-honoured. Sandhi shipped the
  enforcement in `sandhi_server_run_async` at **1.4.9** and archived its filing; daimon collapsed
  onto that call at **1.2.6** (`src/server.cyr:224`). The `"reserved for 0.8.0+"` text still in
  `lib/sandhi.cyr` documents the **sync** options struct, not the async path daimon uses — reading
  a comment near the symbol instead of the code that runs is what produced the wrong conclusion.

`docs/doc-health.md` corrected alongside: it claimed daimon carries no issues directory (false since
1.2.x), and its `SYS_EPOLL_WAIT` tracker is now marked resolved.

### Added — nein firewall MCP tools (`nein_status` · `allow` · `deny` · `validate` · `list` · `diff`)

Wired per nein's own [`mcp-host-integration.md`](https://github.com/MacCracken/nein/blob/main/docs/guides/mcp-host-integration.md)
§"Path B — dispatch by name (recommended for daimon)", a section written against daimon's actual
function names. `/v1/health` now reports **13 MCP tools**, up from 7. Descriptors are read from
nein's own tool table (`nein_tool_count` / `_name` / `_desc`) rather than restated here — a restated
table is a table that drifts — and dispatch routes by **table lookup, not a `nein_` prefix test**, so
a future daimon tool starting with those characters cannot be swallowed.

⛔ **The mutating tools are denied by default, and that is not a placeholder.** `nein_allow` /
`nein_deny` apply **live firewall rules**, and `POST /v1/mcp/call` has **no caller authentication** —
it dispatches on a tool name taken from the request body. bote's `claims` argument, the seam an
identity would arrive through, is a reserved `0` in the 3.x ABI. Registering live firewall mutation
on an unauthenticated endpoint would be a remote root-equivalent hole, strictly worse than the SSRF
and rate-limiter defects this project has already fixed. `daimon_nein_gate` fails **closed** on the
admin set, audits each denial at `SEV_SECURITY`, and classifies via nein's own
`nein_tool_read_only(i)` so daimon hardcodes nothing. Un-gating is roadmap **2.5.x** (agent identity
+ MCP auth). Verified live:

```
nein_status -> {"content":[{"type":"text","text":"nein_status: nft query failed (needs root ...
nein_allow  -> {"content":[{"type":"text","text":"access denied: tool gated by host policy"}],"isError":true}
```

**All three targets still build** — x86_64 · aarch64 · **agnos**. On agnos the tools register and
return nein's specific refusal ("nftables backend unavailable on agnos"), because agnos has its own
network stack and no nft binary. A diagnosable error beats a tool that silently vanishes from the
manifest on one target.

### Fixed (upstream, nein 1.6.12 + 1.7.0) — four defects found by attempting the integration

⛔ **The roadmap said this was "blocked on upstream nein: no `mcp.cyr`, the Rust `mcp.rs` is
unported". That was wrong and is retracted** — nein has shipped the MCP surface since **1.6.0**,
where it was *deliberately redesigned* into flat-arg tools. Nothing was blocked; daimon had not done
the work. What *was* real only surfaced by doing it:

1. **`bridge_config_new` collided with bote's at a different arity** — nein's takes 3 args over a
   32-byte struct (a *network* bridge), bote's takes 2 over 48 bytes (an *HTTP* bridge). cyrius
   refuses to link on that, correctly: the alternative is nein's winning while bote's caller writes
   `bearer_ctx` at +40, past a 32-byte allocation. Fixed **by scope, not rename** — `bridge.cyr` left
   nein's `[lib.mcp]` profile, where nothing referenced it. Non-breaking.
2. **nein's nft-apply path broke the agnos build** — `sys_dup2` and `sys_execve` are *reachable*
   undefined functions there. Fixed by failing closed at the entry point rather than shimming to
   agnos's `exec_redirect`/`spawn_path`, which would compile and then lie: agnos has no nftables to
   drive. Also five 3-arg `sys_waitpid` calls, the same class daimon fixed in 2.1.7.
3. **nein was four cyrius minors behind** (6.6.2) and behind on every shared pin, so its bundle
   carried bote 3.3.7 symbols against daimon's 3.3.13 — 249 `duplicate fn` warnings from version
   skew alone.
4. **nein's error enum was half-namespaced** — four bare `ERR_*` members remained, and one of them
   (`ERR_CHAIN_NOT_FOUND` = 4) collides *by value* with bote's bare `ERR_PARSE` (= 4) in any host
   linking both, which daimon now is. Fixed in **nein 1.7.0**: all four became `NEIN_ERR_*` with
   their values unchanged, so `nein_err_code()`'s integer contract holds and daimon — which names
   none of them — needed no change. Source-breaking for nein's own consumers, hence the minor bump.

⚠ **Residual, stated rather than hidden.** nein declares `[deps.bote-core]` for its own build and
`cyrius deps` resolves it transitively, so daimon — which already vendors the full `dist/bote.cyr` —
carries both packagings and **~249 benign `duplicate fn (last definition wins)` warnings**. Identical
bodies at a matched version, so last-wins is a no-op; the build links and the suite is green. Recorded
in nein's 1.6.12 entry as a packaging question to settle before the next consumer adopts the bundle.

### Changed — dependency pins

| dep | was | now |
|---|---|---|
| **nein** | *(new)* | **1.7.0** |

Declared **after** bote and sigil, deliberately: `dist/nein-mcp.cyr` leaves their symbols unresolved
by design and cyrius resolves forward references in a single pass. Lock: 117 → **119** entries.

### Performance

19 benchmarks unchanged — nein's tools register at `app_init` and dispatch on demand; nothing on a
benchmarked path changed. The binary grows 2,993,176 → **3,196,424 bytes** (+6.8%), the nein bundle's
reachable surface.

## [2.1.7] - 2026-09-22

**daimon builds and runs on AGNOS.** First time in the project's history. Verified by booting it
in ring 3 on a production agnos kernel under QEMU, not by reading the build log. Plus the
issue-tracker close-out: **five filings archived, the sixth resolved here, one left open.**

No toolchain or dependency movement — cyrius stays at **6.6.6** and all eight dep pins are
unchanged. **293 tests** (was 277) across four files, five fuzz harnesses clean, `fmt` / `lint` /
`vet` clean, 21 benchmarks flat.

⛔ **The boot found two real defects the host build had been hiding.** Both are fixed here, and
both were invisible to the whole 277-test suite. That is the argument for the port, independent of
agnos: a second target is a second opinion.

### Fixed — `daimon version` reported `unknown` anywhere but the repo root

The agnos boot printed `daimon vunknown listening on port 8090`. Nothing was wrong with the agnos
build — `daimon_version()` did `file_read_all("VERSION", ...)`, a **relative** path, and fell back
to the string `"unknown"`. That resolves only when daimon is launched from its own source tree,
which is true in development and false everywhere daimon actually ships: an installed `/bin/daimon`,
a container, a service manager with its own working directory.

**The host binary had the identical bug.** `./build/daimon version` only ever looked right because
it was always run from the repo root — and no test could see it, because tests run from the repo
root too. Measured after the fix:

```
from repo root:  2.1.7        # before: 2.1.7 (looked fine)
from /tmp:       2.1.7        # before: unknown
```

`var DAIMON_VERSION` is now compiled in (`src/config.cyr`). The cost is that the number lives in two
files, so **`tests/version_sync.tcyr` (5 assertions) fails the suite if they ever disagree** and
`scripts/version-bump.sh` rewrites both in one step. The duplication is enforced, not documented.

### Fixed — the VULN-009 rate limiter would have failed OPEN on agnos, a third time

`rate_check` opened with `if (ip == 0) { return 1; }  # Can't determine IP, allow`. On aarch64 that
was live for the life of the project (`getpeername` ran as `fchmod`, fixed in 2.1.6). On agnos it
would have been live again by a completely different route: **the kernel exposes no getpeername at
all** — `sock_accept` (#57) returns a raw conn_id and keeps the address behind it — so every caller
would have come back as ip 0 and every request would have been allowed.

The same hole twice, from two unrelated causes, is the argument for closing the *path* rather than
the cause. Unidentified peers now share one bucket and are **still limited**. The ceiling is derived
rather than invented: `RATE_LIMIT_MAX × 8` (the agnos inbound-TCP ceiling) = 960/60s, i.e. exactly
what every simultaneous peer running at its own individual limit would produce — so a legitimate
server is never throttled below its per-IP entitlement, while a flood from one unidentifiable source
meets a ceiling instead of none.

### Added — the AGNOS port

AGNOS is daimon's **primary** target: daimon *is* the AGNOS agent orchestrator and every consumer is
an AGNOS agent. Through 2.1.6 it had never built for it.

**Verified end to end.** `build/daimon-agnos` (2,948,952 bytes) seeded onto an ext2 rootfs and booted
on a *production* agnos kernel (no selftest hook) under QEMU + gnoboot + OVMF + NVMe, exec'd by
kybernet as PID 1 in ring 3:

```
[    6.242024] kybernet: exec /bin/agnsh
[2747164000] [INFO] daimon listening
  daimon v2.1.7 listening on port 8090 (sync)
```

That is allocator init, args init, `app_init`, the sakshi logging path and the server banner, all
running on agnos. (`syscall: stub 258 bytes of 2048` in the same tail is a kernel boot diagnostic
reporting trampoline headroom — unrelated to daimon.)

**The portability layer is `src/syscalls.cyr`**, one file a test can include on its own:

| shim | why it exists |
|---|---|
| `daimon_rename` | agnos carries an **explicit-length invariant** — every path arg carries its length — so `sys_rename(old, oldlen, new, newlen)`. Through 2.1.5 `src/memory.cyr` *compiled* on agnos against daimon's own `SYS_RENAME = 82` while agnos's rename is **31**: the atomic-write path issued a silently wrong syscall. |
| `daimon_unlink` | same invariant (moved here from `src/error.cyr`). |
| `daimon_reap` | the two kernels disagree on arity **and meaning**. Linux `waitpid(pid,&st,WNOHANG)` answers pid/0/-ECHILD; agnos `waitpid` (#4) is *already* non-blocking and answers exit_code/**-2 (WOULD_BLOCK)**/-1. Normalised to 1/0/-1. ⛔ A reaped agnos child's exit code may legitimately be **0**, so "reaped" is `>= 0`, not `> 0` — testing `> 0` would report every cleanly-exiting agent as still running and `agent_stop` would SIGKILL a process that had already exited. |
| `daimon_reap_wait` | agnos has no blocking wait (#4 is a poll; the blocking form is deferred upstream pending per-proc kernel stacks). The host arm now polls too, deliberately: the old `sys_waitpid(pid,&st,0)` blocked **forever** if the child sat in uninterruptible sleep, hanging the supervisor on one wedged agent. A bounded poll degrades that to a leaked zombie the next `agent_is_alive` sweep reports. |
| `daimon_peer_ip` | agnos has no getpeername; returns 0 meaning **unidentified**, which `rate_check` now treats as "limit via the shared bucket" rather than "allow". |

`tests/syscall_portability.tcyr` grew 28 → **39 assertions** covering every shim, and is run on
x86_64 **and** aarch64 under qemu — 39/39 on both.

⚠ **Scope, stated precisely.** daimon's process-spawn and IPC subsystems are **unreachable from any
entry point on every target** — `agent_spawn_with_limits`, `agent_start`, `agent_ipc_bind`,
`ipc_send` and `msg_bus_publish` all have **zero callers**; the HTTP API's agent-create handler calls
`agent_handle_new` and registers a record without spawning a process. So the seven `undefined
function` warnings in the agnos build (`sys_execve`, `sys_socket`, `sys_bind`, `sys_listen`,
`sys_accept4`, `sys_connect`, `sys_pidfd_open`) sit in code **nothing reaches on Linux either**, and
DCE NOPs them. **The agnos build has the same functional surface as the host build** — that is the
honest claim, and it is not "agent spawning works on agnos". When spawn/IPC are wired to the API,
the agnos work resumes: `sys_spawn_path` (#43, which **does** tokenize argv) for exec, and `chan_op`
(#97) capability channels for IPC — an unnamed pair whose one end `CH_ENDOW` places into the spawned
child, which the kernel notes *"deletes the entire unlink-before-bind race class AF_UNIX carries."*
Two constraints are already measured and recorded in the filing: `CH_SEND` caps a payload at **64
bytes** against daimon's `MAX_MESSAGE_SIZE` of 65536 (bulk needs `sys_shm_*`), and **agnos has no
rlimit syscall at all**, so VULN-010 has no direct equivalent there.

### Changed — issue tracker closed out

Every filing was re-verified against the current tree rather than trusted. **Five archived**, the
agnos one resolved here, leaving **one open**.

| filing | outcome |
|---|---|
| `2026-09-14-daimon-does-not-build-for-agnos` | ✅ **RESOLVED here** — builds and boots. |
| `2026-09-14-aarch64-binary-issues-x86-syscall-numbers` | ✅ archived (resolved 2.1.6). |
| `2026-09-22-rag-chunks-alias-the-request-buffer` | ✅ archived (resolved 2.1.6). |
| `2026-06-11-mcp-registry-aliases-request-buffer` | ✅ archived (resolved 1.2.5). |
| `2026-07-03-cyrius-alloc-reset-no-zero-reused-memory` | ✅ archived — **had been marked OPEN since 2026-07-03 while daimon's own roadmap recorded it CLOSED since 1.3.4.** Re-verified at the 6.6.6 pin: `alloc_reset` scrubs the reused span (`lib/alloc.cyr:296-298`), the fix's own comment naming this exact class. ⛔ This closes the **structural** half of VULN-007 only — per-agent arena isolation remains the open half. |
| `2026-07-17-bote-aarch64-sys-open-urandom` | ✅ archived — bote 3.2.0 moved to `SYS_GETRANDOM`; verified at the vendored 3.3.13 (zero non-comment `syscall(SYS_OPEN` sites) and by the aarch64 cross-build succeeding. |

Also corrected in `docs/doc-health.md`: it claimed *"Daimon does not carry its own
`docs/development/issues/` directory"* — false since 1.2.x. Its `cyrius § SYS_EPOLL_WAIT` tracker is
marked resolved (`lib/async.cyr` no longer references the symbol); the `sandhi § max_conns` tracker
stays open (`lib/sandhi.cyr` still reads *"reserved for 0.8.0+"*).

### Performance

19 benchmarks, medians of 6 runs against the 2.1.6 build: **all flat within ±5%** except
`config_default` at +5.8%, which is one 158 ns outlier against five samples of 87–93 ns (2.1.6:
86–93 ns). It is noise **by construction**, not merely by inspection: `tests/daimon.bcyr` mirrors
`src/config.cyr` rather than including it, so this release's only change to that module —
`daimon_version()` — cannot reach the benchmark at all. The two `rag_ingest.bcyr` benchmarks added
at 2.1.6 are unchanged.

## [2.1.6] - 2026-09-22

**Two security fixes and the sweep that closes the class behind each.** A P1 cross-request data
leak in the RAG store, and the two aarch64 syscall defects that turned VULN-009 and VULN-010 off
for the life of the `daimon-aarch64` artifact. No dependency or toolchain movement — cyrius stays
at **6.6.6** and all eight dep pins are unchanged from 2.1.5.

**277 tests** (was 235) across three files, five fuzz harnesses clean, `fmt` / `lint` / `vet`
clean, 21 benchmarks (was 19). Both fixes are **mutation-proven** — each new test file was re-run
against a build with the fix reverted, and each fails there.

### Fixed — P1: RAG queries returned other clients' request bodies

`POST /v1/rag/ingest` retained `str_sub` **views** of the request body as the stored chunk text and
metadata. `str_sub` shares the parent's data pointer by contract (`lib/str.cyr`: *"this is a view,
not a copy"*), and `src/server.cyr` hands those views in from the per-connection buffer sandhi
**reuses** — so a later request overwrote the stored record and the next query served it back:

```
ingest {"text":"Daimon is the AGNOS agent orchestrator. …"} → {"chunk_ids":[1]}
query  {"query":"what orchestrates AGNOS agents?"}
  2.1.5 → "[1] hat orchestrates AGNOS agents?\"} rator. It supervises agents …"
  2.1.6 → "[1] Daimon is the AGNOS agent orchestrator. It supervises agents …"
```

`src/rag.cyr` now `str_clone`s both retained fields. This is the bug **thoth filed in June and
daimon fixed at 1.2.5 for the MCP registry** — `src/mcp.cyr` carries two ⚠ blocks calling it *"a
documented CVE-class bug in this very file"* and instructing *"`str_clone` EVERY FIELD"*. That
sweep never reached `src/rag.cyr`. The convention comment now lives at both sites so a third module
inherits the rule instead of rediscovering it.

⚠ **Not a 2.1.5 regression** — reproduced identically on 6.6.4 and 6.6.6 builds of the same tree
before the fix. Filed at
[`2026-09-22-rag-chunks-alias-the-request-buffer.md`](docs/development/issues/archive/2026-09-22-rag-chunks-alias-the-request-buffer.md),
now **RESOLVED**.

### Fixed — VULN-009 and VULN-010 were silently OFF on every `daimon-aarch64` ever shipped

daimon declared **11 `var SYS_* = <x86_64 number>` globals**. daimon's source is parsed *after* the
auto-prepended stdlib, so each one **overrode the arch-aware peer's value for the whole translation
unit — including inside the stdlib's own `sys_*` wrappers**. Seven numbers happen to be `ESYSXLAT`
rows and were renumbered at runtime; two were not:

| | daimon spelled | aarch64 ran | effect |
|---|---:|---|---|
| `SYS_GETPEERNAME` | 52 | `fchmod` — **succeeds**, buffer untouched | `get_peer_ip` → 0, and `rate_check`'s `if (ip == 0) { return 1; }` allowed **every** request. VULN-009 off. |
| `SYS_SETRLIMIT` | 160 | `uname` — `-EFAULT`, unchecked | agents exec'd with no `RLIMIT_AS` / `RLIMIT_CPU`. VULN-010 off. |

**The sweep:** all 11 globals are gone. **40 raw `syscall(SYS_*, …)` sites → 0**; `sys_*` wrapper
calls go **5 → 43**. Four raw syscalls remain, and every one is namespaced `_DAIMON_SYS_*` in the
new `src/syscalls.cyr` — a prefix that **cannot** shadow a peer symbol, which is the actual root
cause rather than the eleven symptoms. Numbers were read out of the kernel UAPI headers per arch
(`asm/unistd_64.h`, `asm-generic/unistd.h`), not from memory.

- **`setrlimit` → `prlimit64`**, not an `#ifdef` around `setrlimit`: the generic (aarch64) ABI
  supersedes get/setrlimit with `prlimit64` outright — `asm-generic/unistd.h` says so in a comment
  — so one code path with a per-arch number (302 / 261) serves both arches. `struct rlimit64` has
  the same 16-byte `{cur, max}` layout, so the buffers are unchanged.
- **The return is now CHECKED.** A limit that silently fails to apply is the whole point of the
  filing. On failure the child exits **126** rather than exec'ing unconfined — distinct from the
  127 used for a failed exec, so the parent can tell *"could not be confined"* from *"could not be
  started"*.
- `SYS_ACCEPT` 43 → `sys_accept4(fd, 0, 0, 0)` (there is no bare `accept` on aarch64);
  `SYS_RENAME` 82 → `sys_rename`, whose aarch64 arm routes to `renameat(AT_FDCWD, …)`;
  `SYS_WAIT4` → `sys_waitpid`.

**aarch64 `duplicate symbol 'SYS_…'` warnings: 5 → 0.**

**Verified by running it, not by reading it.** The filing asked for *"a 429 after `RATE_LIMIT_MAX`
requests from one IP"*; under `qemu-aarch64` the cross-built binary serves `/v1/health` and, over
130 requests from one IP, returns **119 × 200 then 11 × 429** — the limiter engaging at exactly
`RATE_LIMIT_MAX = 120`. Before the fix `ip == 0` for every caller, so all 130 would have been 200.

[The P2 filing](docs/development/issues/archive/2026-09-14-aarch64-binary-issues-x86-syscall-numbers.md) is
**RESOLVED** on the daimon side; its majra half closed at 2.1.5.

### Added — tests that can actually see these classes

⚠ **`tests/daimon.tcyr`, `tests/daimon.bcyr` and the fuzz harnesses include no `src/` file.** They
reimplement simplified copies of the functions under test — the test file's `rag_ingest_text` never
calls `vec_entry_new` or `vindex_insert` at all, and `bench_rag_ingest_5k` calls only `chunk_text`.
That is *why* both defects shipped green: no assertion and no benchmark reached the code that was
wrong. The three new files include `src/` directly.

- **`tests/syscall_portability.tcyr`** (28 assertions) — asserts **arch-relative behaviour**: it
  calls each syscall and checks the kernel did the thing, never that a number equals a literal.
  That is the only shape that catches the original defect, which compiled clean and ran correctly
  on x86_64. `prlimit64` must read, write and restore `RLIMIT_NOFILE`; `getsockopt` must report
  back `SO_TYPE`; `getpeername` on an **unconnected** socket must fail `-ENOTCONN` — `fchmod`
  returns 0 there, which is exactly how the defect hid. It also round-trips a real AF_UNIX socket
  through the eight wrappers `src/ipc.cyr` converted (bind / listen / connect / accept4 / write /
  read / close) plus the `SO_PEERCRED` check VULN-006 runs on every accepted connection — that file
  needs the libro bundle to include standalone, so it had no other coverage. **28/28 on x86_64 and
  on aarch64 under `qemu-aarch64`.** Against a mutant restoring the 2.1.5 shadowing: **9 of 17 of
  the syscall-number assertions fail** on aarch64.
- **`tests/rag_alias.tcyr`** (14 assertions) — the 1.2.5 clobber shape applied to RAG: ingest from
  a mutable heap buffer, overwrite every source byte as a later request would, assert the store and
  the query path still read back the original. Also pins that `chunk_text` **still returns views**,
  so a future "fix" cannot make chunking allocate per window. Against the pre-fix code: **6 of 14
  fail**.
- **`tests/rag_ingest.bcyr`** (2 benchmarks) — `rag_ingest_real_5k` exercises the true retention
  path; `rag_chunk_only_5k` is the control that isolates the delta.
- **`src/syscalls.cyr`** — the owned numbers, split out of `src/main.cyr`'s preamble so a test can
  include them without dragging in the server. A number no test can reach is how this shipped.
- **CI** — the aarch64 step now **fails** on any `duplicate symbol 'SYS_` warning (the build exits
  0 either way, so only this check can see it), and a new best-effort step runs
  `syscall_portability` under `qemu-aarch64`.

### Performance

**The 19 existing benchmarks are flat — and they cannot see either change**, because they mirror
`src/` rather than include it. Stated rather than presented as coverage. Medians of 8 runs against
the 2.1.5 build, same machine: every target within ±5%, the one mover `vector_insert_100x128d`
**−5.1%**, which also retires the +7.3% flagged at 2.1.5 as the layout noise it was.

The cost of the RAG fix is measured on the path that actually changed, interleaved, 7 pairs:

| | unfixed (aliasing) | fixed (`str_clone`) | Δ |
|---|---:|---:|---:|
| `rag_ingest_real_5k` | 108.97 µs | 120.01 µs | **+10.1%** |

≈11 µs per 5 KB ingest — twelve 512-byte chunk copies plus their allocations. Paid once per ingest,
on the endpoint that was returning other clients' data; `rag_query_text` is untouched. The syscall
sweep is not measurable: it replaces a raw `syscall` with a wrapper that emits the same instruction.

### Known issues (carried)

Unchanged from 2.1.5 and re-checked here: `duplicate fn 'uname_release'` and the static-array
warning (both upstream in sigil), and the `memory_store_get` *"SINGLE value"* diagnostic, still a
documented false positive on dead code.

The **`--agnos` build still fails, at 3 errors** — and the 2.1.5 characterisation of this as a "scoping decision" was wrong enough to correct here. **AGNOS is daimon's primary target**, and the agnos kernel supplies a primitive for everything daimon does; the gap is that daimon has never been mapped onto them, behind the `#ifdef CYRIUS_TARGET_AGNOS` pattern seven sibling repos already use. The sweep changed the error *class* for the better: 2.1.5's undefined `SYS_EXECVE` / `SYS_WAIT4` resolve now, and what surfaces is that the agnos peer spells these differently — `sys_waitpid(pid)` takes 1 arg, `sys_rename(old, oldlen, new, newlen)` takes 4 (agnos carries an explicit-length invariant: every path argument carries its length). More importantly `src/memory.cyr` previously compiled on agnos against daimon's `SYS_RENAME = 82` while agnos's `rename` is **31** — a silent wrong-syscall, now a loud compile error, which is the same win as the aarch64 half of this release. The mapping and its two real constraints (chan's 64-byte payload cap vs daimon's 64 KB messages; agnos having no rlimit syscall) are written up in the roadmap and the filing. Raised to **P1**.

## [2.1.5] - 2026-09-22

**Toolchain `6.6.4` → `6.6.6` plus three dependency pins; the other five were already at their
latest tags.** No daimon source change — this is a pin-only release. **235 tests** pass, five fuzz
harnesses clean, `fmt` / `lint` / `vet` clean (29 deps, 0 untrusted), `cyrius.lock` verifies 117/0
with the `cyrius	6.6.6` trailer. Benchmarks measured head-to-head against a 6.6.4 build of the
same tree, **10 interleaved trial pairs**: 15 of 19 flat, three faster, one slower — detail below.

Two portability items moved without a line of daimon code changing, and one **pre-existing P1 data
leak was found** by running the binary. Both are recorded rather than acted on here.

### Changed — dependency pins

| dep | was | now | | dep | was | now |
|---|---|---|---|---|---|---|
| cyrius | 6.6.4 | **6.6.6** | | sakshi | 2.5.2 | 2.5.2 |
| libro | 2.10.1 | **2.10.3** | | bayan | 1.5.6 | 1.5.6 |
| majra | 2.7.2 | **2.9.1** | | sigil | 3.12.18 | 3.12.18 |
| bote | 3.3.9 | **3.3.13** | | samay | 1.1.2 | 1.1.2 |
| | | | | ai-hwaccel | 2.3.23 | 2.3.23 |

The transitive `patra` pin moves 1.14.1 → **1.14.3** in the lock, which is the version the stdlib
snapshot already vendored — the 2.1.4 mismatch noted under "refusing to overwrite stdlib leaf" is
gone. Lock grows 116 → 117 entries. `lib sync --full` copies 111 files (was 110); the new one is
`alloc_cx.cyr`, the allocator peer for the cx bytecode target, which nothing in daimon reaches.

What each moved pin carries for daimon:

- **cyrius 6.6.6** — the 6.6.5+6.6.6 repair line. The three hard-error classes it adds were audited
  against `src/` before the bump and none has a site: no `ret2`/`rethi` pair-return with a
  single-value branch, no top-level `{ }` block `var`, and no duplicate top-level global (`grep '^var '`
  across `src/*.cyr` → 50 globals, zero duplicated names). The roadmap's pre-flight prediction that
  `lib/assert.cyr`'s new transitive `lib/vec.cyr` include would not collide with
  `src/vector_store.cyr`'s `vec_entry_*` family held — the prefix is shared, the fourteen exported
  `vec_*` names are not.
- **majra 2.9.1** (two minors, from 2.7.2) — **closes the upstream half of the aarch64 filing.**
  majra 2.7.2's bundle declared `var SYS_GETRANDOM = 318;` (x86_64), which is not an `ESYSXLAT` row
  and, being prepended after the stdlib leaves, overrode the aarch64 peer's 278 **for daimon's whole
  translation unit**. 2.9.1 routes `uuid_generate` through the per-target `sys_getrandom` wrapper, and
  the sixth duplicate-symbol warning is gone from daimon's aarch64 build. majra remains
  compile-time-only for daimon; nothing calls it at runtime.
- **bote 3.3.13** — answers `ping` with `{"result":{}}` instead of `-32601` (SDK clients read the
  error as an unhealthy server), accepts MCP revision `2025-06-18`, and adds `src/sandbox.cyr` to
  `dist/bote.cyr` (30 → 31 modules). daimon calls the `libro_*` handlers directly, so the dispatcher
  fixes do not reach its MCP surface; the bundle change is the reachable part.
- **libro 2.10.3** — repair releases behind the audit chain daimon seeds at `daimon_audit_init`.

### Changed — `--agnos` build: 53 errors → 3, and the upstream blocker is closed

The roadmap item read *"P2 (upstream cyrius/bote + 3 daimon sites) — `--agnos` build fails, 53
errors"*, rooted at 2.1.4 in bote's `dist/bote.deps` sidecar naming the Linux-internal
`syscalls_linux_common` as a leaf, which `cyrius deps` prepended target-blind so the Linux peer
compiled beside the standalone agnos peer. **Measured at 6.6.6: three errors, and
`syscalls_linux_common` does not appear in the agnos unit at all.** What remains is exactly daimon's
own share, already named in the filing:

```
error:src/agent.cyr:262:27: undefined variable 'SYS_EXECVE'
error:src/agent.cyr:321:35: undefined variable 'SYS_WAIT4'
error:src/agent.cyr:324:30: undefined variable 'SYS_WAIT4'
```

The item is no longer upstream-blocked; it is a scoped daimon change (an agnos spawn arm) awaiting
the `dynlib`/`fdlopen`/`tls`/`mmap`/`net` scoping decision the filing calls for. Not done here — a
pin release is the wrong place for it.

### Fixed (upstream) — the aarch64 asset's sixth warning; its two real defects are unchanged

The aarch64 cross-build succeeds and produces a valid ELF. The `ESYSXLAT` routed set was re-derived
**by disassembling the shipped `build/daimon-aarch64`** rather than read off a changelog: **60 rows**
at 6.6.6, up from 44 — confirming the growth, and confirming what it does *not* include.

| symbol | daimon | routed at 6.6.6 | effect on aarch64 |
|---|---:|---|---|
| `SYS_SOCKET` / `CONNECT` / `ACCEPT` / `BIND` / `LISTEN` / `GETSOCKOPT` | 41/42/43/49/50/55 | → 198/203/202/200/201/209 | correct ✓ |
| `SYS_RENAME` | 82 | → 38 (`renameat`) | correct ✓ |
| `SYS_PIDFD_OPEN` / `SYS_PIDFD_SEND_SIGNAL` | 434/424 | pass-through (unified numbering) | correct ✓ |
| **`SYS_GETPEERNAME`** | **52** | **still not a row** | runs as `fchmod` — VULN-009 per-IP rate limiter off |
| **`SYS_SETRLIMIT`** | **160** | **still not a row** | runs as `uname` — VULN-010 agent rlimits never applied |

Neither of daimon's two broken numbers was among the sixteen added, so
[the P2 filing](docs/development/issues/archive/2026-09-14-aarch64-binary-issues-x86-syscall-numbers.md)
stays open and unchanged on the daimon side. Its majra half is now resolved (above).

### Note — a P1 cross-request data leak, found by running the binary

The roadmap's own post-bump check ("one agent lifecycle through the HTTP API and a vector-store
round trip") turned up a **pre-existing** defect the 235-assertion suite cannot see:
`POST /v1/rag/ingest` stores chunk text and metadata as `str_sub` **views** into the per-connection
request buffer, which `src/server.cyr:210` documents as a *reused* per-batch arena. A later request
overwrites those bytes, so `POST /v1/rag/query` returns **another client's request body**:

```
ingest {"text":"Daimon is the AGNOS agent orchestrator. …"} → {"chunk_ids":[1]}
query  {"query":"what orchestrates AGNOS agents?"}
    → "[1] hat orchestrates AGNOS agents?\"} rator. It supervises agents …"
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ the query body, in the stored record
```

This is a recurrence of the HIGH bug thoth filed in June and daimon fixed at 1.2.5 for the MCP
registry — `src/mcp.cyr` carries two ⚠ blocks calling it *"a documented CVE-class bug in this very
file"* and instructing *"`str_clone` EVERY FIELD"*. That fix was never swept to the other retaining
stores (`grep -c str_clone`: `mcp.cyr` 16, `rag.cyr` 0). **Reproduced identically on 6.6.4 and 6.6.6
builds of the same tree, so it is not a bump regression** — recorded, not fixed here, because it
needs its own test and bench cycle. Filed as
[`2026-09-22-rag-chunks-alias-the-request-buffer.md`](docs/development/issues/archive/2026-09-22-rag-chunks-alias-the-request-buffer.md)
(P1) and roadmapped. The fix is one call site (`src/rag.cyr:140`); the regression-test pattern
already exists in-repo from 1.2.5.

### Performance

19 benchmarks, `build/daimon_bench` from this tree built twice — once at the 2.1.4 pins, once at
2.1.5's — then run **interleaved, 10 trial pairs**, so machine drift hits both equally. Median of
per-trial means; `Δmin` is the best single sample, which is the floor with scheduler noise removed.

| benchmark | 6.6.4 | 6.6.6 | Δmed | Δmin |
|---|---:|---:|---:|---:|
| config_default | 96ns | **88ns** | −8.4% | −3.4% |
| mcp_find_tool_in_100 | 96ns | **90ns** | −6.7% | −5.3% |
| cosine_128d | 641ns | **608ns** | −5.2% | +0.0% |
| vector_insert_100x128d | 24.306us | 26.080us | **+7.3%** | +2.7% |
| rag_ingest_5k_chars | 4.114us | 4.298us | +4.5% | +1.9% |
| *(14 others)* | | | within ±3% | within ±3%, except `mcp_manifest_100_tools` +4.8% |

`vector_insert_100x128d` is the one regression and the noisiest target in the set — 100 iterations
against a ~20.6 µs floor with tail samples from 33 µs to 92 µs. Its **floor moved +2.7% while its
median moved +7.3%**, i.e. most of the gap is tail distribution, not per-op cost. No daimon source
changed, so this is code layout and the compiler's own growth (the x86_64 binary is 2,963,584 →
2,993,192 bytes, +1.0%, which cyrius 6.6.6 attributes to its new refusals, the checked write path
and the PE flag decoder). Recorded as measured rather than written off as noise.

### Known issues (carried, re-checked at 6.6.6)

- `duplicate fn 'uname_release'` — `lib/sigil.cyr:746` vs `lib/sys.cyr:203`. Upstream (sigil),
  harmless (identical one-line bodies, last-wins is a no-op), unchanged since 2.1.4.
- `array local over the per-fn frame budget gets STATIC storage` — `lib/sigil.cyr:25118`, sigil's
  `var buf[262144]` crypto-bank init. Always had static storage; 6.6.5 promoted the note to a
  warning. Upstream.
- The `memory_store_get` *"returns a `: stack` pair … but a SINGLE value here"* diagnostic is still a
  **false positive**, and the 2.1.4 note's reasoning was re-verified independently this release: a
  16-line repro of the canonical `None()`-on-one-path / `Some(v)`-on-another shape reproduces the
  warning **and all four runtime assertions hold** (`is_some`=1, payload=50, `is_none`=1,
  `is_some`=0). `lib/tagged.cyr` documents the shape as correct; cyrius's own 6.6.0 entry introducing
  the diagnostic documents its false-positive class and says *"the shape is only a defect when every
  path is meant to be a Result — read the callers before acting on it."* A/B'd across 6.6.4 / 6.6.5 /
  6.6.6: identical at all three, so it is not new. `memory_store_get` has no callers and is DCE'd.
  Left as-is.

## [2.1.4] - 2026-09-14

**Toolchain `6.6.2` → `6.6.4` plus six of the eight dependency pins.** No daimon source
migration was needed — 6.6.3/6.6.4 are repair releases (lexer, visibility, `continue`
binding, DCE data-vaddr, lock determinism), and daimon uses none of the `private`/`public`
file-visibility surface those releases tightened. **235 tests** pass, five fuzz harnesses
clean, all 19 benchmarks flat within noise against a clean 2.1.3 build under 6.6.2.

### Fixed — `agent_ipc_bind` called a function that does not exist

The 2.1.3 tree's uncommitted-then-committed `daimon_unlink` shim (the agnos arity fix
filed at `docs/development/issues/archive/2026-09-14-daimon-does-not-build-for-agnos.md`) had one
call site spelled `daimon_unlink(path_cstr, cstr_len(path_cstr))` at `src/ipc.cyr:183`.
There is no `cstr_len` in the stdlib — the build printed `warning: undefined function
'cstr_len'` and linked anyway. Now `str_len(spath)`: the `Str` is already in scope two
lines up, it matches the other three call sites, and it avoids the scan the shim's own
comment says the host arm should not pay.

### Changed — dependency pins

| dep | was | now | | dep | was | now |
|---|---|---|---|---|---|---|
| cyrius | 6.6.2 | **6.6.4** | | sigil | 3.12.16 | **3.12.18** |
| sakshi | 2.5.1 | **2.5.2** | | libro | 2.10.0 | **2.10.1** |
| ai-hwaccel | 2.3.22 | **2.3.23** | | majra | 2.7.2 | 2.7.2 |
| samay | 1.1.2 | 1.1.2 | | bote | 3.3.8 | **3.3.9** |

samay and majra are already at their latest tags. The transitive `patra` pin (via libro)
moves 1.13.10 → 1.14.1 in the lock; the vendored `lib/patra.cyr` reads 1.14.3 because the
6.6.4 stdlib snapshot ships it and the resolver keeps the snapshot leaf over the dep
artifact (the pre-existing "refusing to overwrite stdlib leaf" note).

What each pin carries for daimon:

- **cyrius 6.6.4** — `cyrius.lock` now ends with a `cyrius	6.6.4` trailer and is written
  in sorted order (6.6.3), so the committed lock verifies on any machine and a stdlib leaf
  whose snapshot bytes move under an unchanged pin is refused rather than silently
  re-locked. This is the class the 2.1.3 tree hit: `cyrius build` rewrote the
  `lib/ai-hwaccel.cyr` hash on every run while `git status lib/` stayed clean. Requires the
  6.6.4 wrapper — pre-6.6.4 `deps --verify` skips the trailer (it is last by design).
- **sigil 3.12.18** — `agnosys_uname` moves from a raw `syscall(63, …)` (x86_64 `uname`;
  `read(2)` on aarch64) to `sys_uname` from `lib/sys.cyr`, which the sidecar now pulls.
  ⚠ Two definitions of `uname_release(uts)` now land in one translation unit —
  `lib/sys.cyr:203` and sigil's bundled copy at `lib/sigil.cyr:746` — and the compiler
  reports `duplicate fn 'uname_release' (last definition wins)`. Both bodies are the same
  one-liner (`return uts + UTS_RELEASE;`), so last-wins is a no-op; it is an upstream
  packaging duplicate for sigil to drop, not a daimon defect. Noted so the next bump
  knows it is expected.
- **libro 2.10.1** — fixes the `--features tpm` arm that could not compile under the value
  form. daimon does not build that arm.
- **bote 3.3.9** — retires bote's `ulimit -v` distlib workaround now that cyrius 6.6.3
  fixed the ~20 GB leaf-lookup allocation. No bundle change that reaches daimon.
- **sakshi 2.5.2 / bayan 1.5.6 / ai-hwaccel 2.3.23** — toolchain re-verification
  releases, no source change.

### Benchmarks — 6.6.2 (clean 2.1.3 checkout) vs 6.6.4 (this tree), same machine

| bench | 6.6.2 | 6.6.4 | | bench | 6.6.2 | 6.6.4 |
|---|---:|---:|---|---|---:|---:|
| config_default | 123ns | 98ns | | mcp_extract_input_schema | 5.064µs | 5.290µs |
| cosine_128d | 727ns | 676ns | | edge_register_100 | 190.235µs | 189.184µs |
| vector_insert_100x128d | 25.183µs | 26.822µs | | edge_heartbeat_100 | 721.735µs | 724.549µs |
| vector_search_1k_64d | 322.525µs | 323.981µs | | edge_stats_500 | 49.008µs | 53.827µs |
| rag_ingest_5k_chars | 4.297µs | 4.218µs | | circuit_breaker_cycle | 4.046µs | 4.048µs |
| scheduler_100_tasks | 186.180µs | 187.436µs | | hashmap_1000_insert_lookup | 493.282µs | 474.718µs |
| supervisor_register_1000 | 375.284µs | 352.994µs | | json_parse | 421ns | 419ns |
| mcp_register_100_tools | 62.516µs | 60.358µs | | secure_zero_4k | 5.410µs | 5.446µs |
| mcp_manifest_100_tools | 68.739µs | 67.067µs | | trace_id_hex | 45ns | 45ns |
| mcp_find_tool_in_100 | 95ns | 105ns | | | | |

Binary 2,959,376 → 2,963,584 bytes (+4,208; 4,906 → 4,927 fns NOPed by DCE).

⚠ The `bench-history.csv` record before this run dated from 2.1.2 under cyrius **6.5.36**
(2.1.3 said "benches flat" but never appended a run), so a naive diff against it showed
`hashmap_1000_insert_lookup` +13% and `edge_stats_500` +10%. Rebuilding 2.1.3 from a clean
checkout under 6.6.2 reproduced both numbers, so that delta belongs to the 6.5.36 → 6.6.2
flip, not to this release. Three repeat runs per side agree.

### Note — the `--agnos` build re-measured under 6.6.4: same 53 errors

The issue filed at 2.1.3 named a stale vendored snapshot as the likely root and re-vendoring
under 6.6.4 as the first thing to try. Tried: **identical 53 errors, 36 distinct symbols**, so
that hypothesis is refuted. The measured root is upstream: bote's `dist/bote.deps` sidecar
names `syscalls_linux_common` as a stdlib leaf (distlib resolved the symbols bote calls —
`sys_accept4` et al. — to the file that *defines* them, which is the Linux-internal peer, not
the `syscalls` dispatch umbrella), and `cyrius deps` prepends every sidecar leaf as a
target-blind top-level include. So on `--agnos` the Linux peer is compiled next to the
standalone agnos peer. Nothing in daimon's own `[deps] stdlib` or sources names it. Recorded
in the issue file; the fix sits in cyrius distlib and/or bote's sidecar.

### Note — the aarch64 artifact silently loses two hardening layers (pre-existing, filed)

`cyrius build --aarch64` exits 0 at both 6.6.2 and 6.6.4 — the CI lane's three "known
upstream blocker" symbols are long resolved — but it prints six `duplicate symbol 'SYS_…'
redefined with conflicting value (last definition wins)` warnings: five daimon globals
(`src/main.cyr:34-42`, `src/server.cyr:11`) and majra 2.7.2's `SYS_GETRANDOM = 318`. Seven of
daimon's nine hand-spelled x86_64 numbers are harmless on aarch64 because the backend's
runtime `ESYSXLAT` chain renumbers them (41/42/43/49/50/55/82 → socket/connect/accept/
bind/listen/getsockopt/renameat) — the binary binds and serves. **Two are not routed and run
as different syscalls:** `SYS_GETPEERNAME` 52 is `fchmod` there — it *succeeds* with an
untouched buffer, so `get_peer_ip` returns 0 and `rate_check` takes its "can't determine IP,
allow" path: the VULN-009 per-IP rate limiter is off for every client. `SYS_SETRLIMIT` 160 is
`uname` — `-EFAULT`, unchecked in the forked child, so agents exec with no RLIMIT_AS/RLIMIT_CPU:
the VULN-010 limits are never applied, and this one emits **no** warning. CI checks only that the
output is an aarch64 ELF. This is the class cyrius 6.6.4 swept from its stdlib and named for the
consumer pin sweep. Not fixed here — filed as
`docs/development/issues/archive/2026-09-14-aarch64-binary-issues-x86-syscall-numbers.md` (P2) and,
for majra's bundle — where the same sweep found `_SYS_FCHMOD` 91 on its IPC bind path and a raw
`syscall(35)` in its DAG backoff, both unrouted — as majra
`docs/development/issues/2026-09-14-raw-x86-syscall-numbers-aarch64.md`. ⚠ The first cut of
this note said the aarch64 asset "cannot bind a socket"; that was wrong and was corrected after
verifying against the emitter's routing table.

### Note — a diagnostic false positive on dead code

6.6.2+ prints `warning:src/memory.cyr:102:19: memory_store_get returns a : stack pair on
another path but a SINGLE value here` for `return None();`. Per `lib/tagged.cyr`'s own note
a nullary variant returns its tag alone and is *correctly* not pair-returning, so
`None()` on one path + `Some(v)` on another is the documented shape; the diagnostic keys on
the callee's pair flag, which `None` does not carry. `memory_store_get` has no callers and
is DCE'd. Left as-is rather than restructured around a compiler warning.

## [2.1.3] - 2026-09-11

**Toolchain `6.5.36` → `6.6.2` — the `Result` / `Option` / `Either` value form — plus
all eight dependency pins.** **235 tests** pass, five fuzz harnesses clean, benches
flat.

### Fixed — `memory_store_set` rejected a key and reported success

⛔ **A genuine fail-open, and the reason this is a Fixed.** `src/memory.cyr:61` read:

```
var vr = validate_key(key);
if (is_err_result(vr) == 1) { return vr; }
```

Under the value form `vr` binds the **tag alone**, so `return vr;` handed the caller
the payload by itself — a rejected key arrived as `tag = <payload>`,
`is_err_result == 0`. **An error that reads as SUCCESS.** `validate_key` is the guard
on the agent-memory key namespace, so a key it refused was reported to the caller as
stored. Now rebuilt from both halves with `return Err(vr_v);`.

### Changed — dependency pins

| dep | was | now | | dep | was | now |
|---|---|---|---|---|---|---|
| cyrius | 6.5.36 | **6.6.2** | | sigil | 3.12.14 | **3.12.16** |
| sakshi | 2.4.11 | **2.5.1** | | libro | 2.8.12 | **2.10.0** |
| ai-hwaccel | 2.3.20 | **2.3.22** | | majra | 2.7.1 | **2.7.2** |
| samay | 1.0.1 | **1.1.2** | | bote | 3.3.7 | **3.3.8** |

The **bote** bump closes the loop on the collision daimon itself reported at 2.1.1.
majra 2.7.1 renamed `_sub_new` → `_majra_sub_new`, but bote ≤ 3.3.7 still pinned majra
**2.7.0**, so any consumer reaching majra *through* bote — daimon does — kept inheriting
it. bote 3.3.8 re-pins majra 2.7.2. Separately, daimon consumes the **full**
`dist/bote.cyr`, which at 3.3.7 called bare `payload(` from `src/transport_unix.cyr`;
that symbol does not exist in the 6.6.2 stdlib, so the build could not have linked.

### Changed — value-form migration

Nine sites. Five were reachable by scanning for the deleted `payload()` accessor
(`src/agent.cyr`, `src/api_edge.cyr`, `src/api_sched.cyr`, and two in
`tests/daimon.tcyr`); **the other four were not**, because they bind a `Result` and
test it without ever unwrapping — `edge_fleet_heartbeat`, `edge_fleet_decommission`,
`task_scheduler_cancel_task` and four test sites. They surfaced only from the
compiler. An accessor grep is not a migration survey.

### Added — a guard in `api_sched_submit`

`POST /v1/scheduler/tasks` took the result of `task_scheduler_submit_task` and used
its payload as the task id with **no error check**, while `api_edge_register` — the
structurally identical endpoint eight lines away in `api_edge.cyr` — has always
guarded. Added for symmetry.

⚠ **Honest scope:** this is defensive, not a fix. samay's
`task_scheduler_submit_task` returns `Ok(id)` on every path today, so the `Err` arm is
currently unreachable. It matters only if samay grows a failure path — at which point
the `Err` payload would have been serialised as the `task_id` in a 201 response.

### Note — `fuzz/` is not run by CI

`fuzz/` holds five harnesses (circuit_breaker, mcp_registry, memory_keys,
scheduler_fsm, vector_store) and neither workflow invokes `cyrius fuzz`. All five pass
today — run by hand for this release — but nothing gates them. bote 3.3.8 found four
harnesses that had silently stopped compiling four minors earlier for exactly this
reason.

## [2.1.2] - 2026-08-30

Picks up the majra fix for the `_sub_new` collision 2.1.1 reported. **235 tests**
pass, lock verifies 114/114, and the 19-benchmark suite shows no regressions.

### Fixed

- **`majra` 2.7.0 → 2.7.1**, which closes the `_sub_new` collision recorded as a
  known issue in 2.1.1. majra renamed its private pubsub helper to
  `_majra_sub_new`, so `libro`'s `_sub_new(pattern)` is no longer silently
  answered by majra's `_sub_new(chan, filter_fn)` — a two-argument call reaching
  a two-parameter function that allocated 40 bytes via `fl_alloc` where 24 via
  `alloc` were meant. **daimon now builds with zero duplicate-fn warnings.**

  majra 2.7.1 also prefixed `sha1`/`_sha1_rotl32`, which collided with the
  stdlib's `lib/sha1.cyr` at a *different arity* (`sha1(data, len, digest_out)`
  against majra's two-argument form). daimon links both, so that one was live
  here too.

### Known issues

- majra's `ws_recv_frame` and `ws_send_text` still collide with the stdlib
  `lib/ws.cyr`, and the implementations differ. They are majra's documented
  public API, so renaming them is a breaking change awaiting a majra minor.
  daimon does not call either, so nothing here reaches the ambiguity.

## [2.1.1] - 2026-08-30

Dependency and toolchain refresh. No source change; **235 tests** pass and the
19-benchmark suite shows no regressions. The build now emits no shadow warning
and its static-data footprint drops by 43%.

### Changed

- **`ai-hwaccel` 2.3.19 → 2.3.20.** 2.3.20 makes ai-hwaccel's bayan dependency
  optional and feature-gated, so it no longer resolves transitively into
  consumers. daimon declares its own `[deps.bayan] 1.5.2`, so this removes a
  duplicate rather than a capability.

- **`sigil` 3.12.9 → 3.12.14**, aligning the declared dep with the version the
  pinned stdlib snapshot ships. This was the larger of the two `./lib/ shadows
  version-pinned lib/` warnings: two copies of sigil were being linked. Static
  data falls **806,976 → 454,720 bytes (−43.6%)**. 3.12.14 also carries the
  fix for 3.12.13's subprocess regression, which broke agnos and Windows builds
  across the ecosystem.

- **`lib/sankoch.cyr` refreshed** 2.7.8 → the pinned 2.7.10. Not a declared dep
  — a stale vendored file, the same drift class as the sigil one. 2.7.10 is the
  release that survives a caller's `alloc_reset()`.

- **Cyrius pin `6.5.35` → `6.5.36`**, matching the installed compiler; `lib/`
  resynced (67 declared modules). Clears the toolchain-drift warning.

### Known issues

- **`_sub_new` is defined by both `majra` and `libro` with different signatures**
  and different semantics — `majra` takes `(chan, filter_fn)` and `fl_alloc`s 40
  bytes, `libro` takes `(pattern)` and `alloc`s 24. Cyrius has one flat function
  namespace, so majra's wins by include order and `libro.cyr:6634`'s one-argument
  call reaches it with an uninitialised second argument and the wrong allocation
  size.

  **Latent in daimon**: nothing in `src/` calls either subscription API, so no
  daimon code path reaches it today. It is not fixable from here — both are
  upstream libraries, and libro 2.10.0 still defines the name — so it needs one
  of the two to adopt a module prefix. Bumping libro two minor versions inside a
  patch release would not have fixed it and was not attempted.

## [2.1.0] - 2026-08-24

### Added

- **MCP resources and prompts reach the spine.** daimon hosted MCP *tools* and nothing else: a consumer
  asking for `/v1/mcp/resources` or `/v1/mcp/prompts` got a 404, so server-published resources and prompts
  were unreachable through daimon no matter what the downstream MCP server offered. bote has served
  `resources/list`, `resources/read`, `prompts/list` and `prompts/get` for some time, and its reference
  server publishes a real resource (`bote://info`) and a real prompt (`bote_greeting`) — daimon was the
  missing hop. Six new endpoints, modelled line-for-line on the tool surface so a client that already
  speaks daimon's tools needs no new idioms:

  | | |
  |---|---|
  | `GET /v1/mcp/resources` | `{"resources":[{uri,name,description,mimeType}],"count":N}` |
  | `POST /v1/mcp/resources` | register an external resource |
  | `POST /v1/mcp/resources/read` | forward MCP `resources/read` to the registering host |
  | `GET /v1/mcp/prompts` | `{"prompts":[{name,description,arguments}],"count":N}` |
  | `POST /v1/mcp/prompts` | register an external prompt |
  | `POST /v1/mcp/prompts/get` | forward MCP `prompts/get` to the registering host |

  Plus `POST /v1/mcp/{resources,prompts}/deregister`, and `mcp_resources` / `mcp_prompts` counts on
  `/v1/health` and the metrics endpoint. Verified end to end against a live bote: registration, listing,
  a `resources/read` returning the resource body, and a `prompts/get` returning a rendered message.

  ⚠ **The URI travels in the request BODY, never in a path segment.** An MCP resource URI contains `://`
  and further slashes (`bote://info`), which would collide with the router's prefix arithmetic and with
  query-stripping, and would need escaping nobody gets right twice. `POST` with a JSON body is what
  `/v1/mcp/call` already does for tool names, so this follows the existing convention rather than adding
  one. Deregistration takes the same shape, on its own path, so a read can never be parsed as a delete.

  ⚠ **Registration, not fan-out aggregation.** The tempting alternative — answer `resources/list` by
  asking every registered host at request time, since a server already knows its own resources — was
  rejected on blocking behaviour. The forward path builds no options struct, so every sandhi timeout
  getter returns 0 and a dropped-SYN host blocks the single sync worker for as long as the OS takes to
  give up. Aggregation would put that cost on *every* list, multiplied by the number of hosts. With
  registration, listing is local and instant and only a read/get — one host, explicitly addressed — can
  block. The underlying unbounded-timeout issue is pre-existing and untouched here.

- **`trace_mcp_forward_method`** — the outbound MCP hop generalized to any method. `sandhi_rpc_mcp_call`
  always took the method as a parameter; only daimon's wrapper had hardcoded `"tools/call"`. Tracing and
  `traceparent` injection stay single-sourced across tools, resources and prompts.

### Fixed

- **The aarch64 cross-build, which had been failing to COMPILE.** `src/ipc.cyr` and `src/memory.cyr`
  called `syscall(SYS_MKDIR, …)`, `syscall(SYS_UNLINK, …)` and `syscall(SYS_CHMOD, …)` directly. arm64
  Linux has no legacy `mkdir`/`unlink`/`chmod` at all — only the `*at` forms — so those constants do not
  exist on that target and the lane died with 7 `undefined variable` errors. A raw `syscall(SYS_FOO, …)`
  is a portability *claim*; the stdlib `sys_mkdir` / `sys_unlink` / `sys_chmod` wrappers are the thing
  that actually resolves per target. Swapped, and the lane goes from 7 errors to **OK** with a valid
  aarch64 ELF. (Noted in-source for future porters: the AGNOS variant of these wrappers takes a `pathlen`
  rather than a mode and its `sys_unlink` takes two arguments — daimon does not target AGNOS, but that
  arity difference is real.)

- **The CI format gate, which could never pass.** It diffed `cyrius fmt`'s stdout against each file, but
  `cyrius fmt <file>` rewrites in place and prints nothing — so the diff compared an empty stream against
  every file and reported drift for all 35, including correctly-formatted ones. Now uses `--check` (exit 1
  on drift, names the file, no stdout by design), the same shape kavach uses. With a gate that can answer
  the question, the real drift was **4 files** — `audit.cyr`, `http.cyr`, `mcp_builtin.cyr`,
  `vector_store.cyr` — reformatted here. **hoosh carries the same broken idiom.**

- **A stale `bayan` vendored at two different releases at once.** `lib/bayan.cyr` read `Version: 1.4.1`
  (the monolithic dist from the pinned toolchain's stdlib snapshot) while `lib/bayan-json.cyr` read
  `Version: 1.5.2` (modular component files pulled by libro/bote/majra's `.deps` sidecars). The result was
  **134 `duplicate fn (last definition wins)` warnings** and an `undefined function 'json_v_parse_str'` —
  the 1.5.2 half calling into a 1.4.1 monolith that no longer exported it. Fixed with an explicit
  `[deps.bayan]` pin at `modules=["dist/bayan.cyr"]`, exactly the remedy the neighbouring `[deps.sigil]`
  block documents for the identical collision. Warning count for the whole build: **137 → 3**, with a
  before/after warning-set diff confirming **zero** new warnings. The three that remain (`_sub_new`,
  `cancel_token_new`, `json_v_parse_str`) are pre-existing and unrelated: the last comes from a stale
  `samay` still calling a name bayan removed at 1.3.0, which `ai-hwaccel` already fixed at 2.3.16.

### Changed

- **Toolchain pin `6.5.27` → `6.5.35`** and **every dependency brought current**: `sakshi` 2.4.10 →
  2.4.11, `ai-hwaccel` 2.3.17 → 2.3.19, `libro` 2.8.5 → 2.8.12, `majra` 2.6.6 → 2.7.0, `bote`
  3.3.1 → 3.3.7. daimon was the last first-party repo two pins behind. Each bump was verified by reading
  the vendored `lib/<dep>.cyr` header rather than trusting the manifest, and `cyrius lib sync --full`
  re-synced the 108-file snapshot. The `bote` bump is not cosmetic — it removed the
  `duplicate fn 'cancel_token_new'` collision against `lib/async.cyr`. Build warnings are now down to
  **two**: `_sub_new` (majra/libro) and an `undefined function 'json_v_parse_str'` from a stale `samay`
  1.0.1 still calling a name bayan removed at 1.3.0 — the same fix `ai-hwaccel` already took at 2.3.16.
  Both are upstream and pre-existing.

### Notes

- Suite **215 → 235**. Per house convention the test file mirrors each new registry function; the added
  group covers URI-with-`://` round-tripping, callback-URL storage, deregistration, and the nested
  `arguments` array — including the two cases that must never emit invalid JSON (absent, and present but
  not an array).
- ⚠ **`cyrius check <file>` REWRITES SOURCE IN PLACE.** Running it here reformatted four files this cut
  never touched (`audit.cyr`, `http.cyr`, `mcp_builtin.cyr`, `vector_store.cyr`) using the *installed*
  6.5.35 formatter rather than the pinned one; the changes were reverted. It is also the wrong gate for a
  manifest project — it does not prepend the manifest's includes, so it reports ~92 phantom
  "reachable undefined function" errors. Use `cyrius vet` + `cyrius build` + `cyrius test`, which is what
  CI runs.
- ⚠ **The nested-JSON trap this cut hit twice, recorded because it is not obvious.** The flat
  `json_parse`/`jget` scanner stops at the first `,` or `}`, so *any* field positioned after a nested
  value is invisible to it — registering a prompt whose body carried `"arguments":[…]` before
  `"callback_url"` failed with "missing callback_url" while the field was plainly present. And
  `bayan_json_v_parse` takes a **`Str`**, not a cstr: handing it `str_cstr(body)` reads a char pointer as
  a `Str` header and **segfaults the server**. Both were caught by live testing, not by review.

## [2.0.2] - 2026-08-18

### Changed

- **Dependency set brought current: `sakshi` 2.4.6 -> 2.4.10, `ai-hwaccel` 2.3.15 -> 2.3.17,
  `sigil` 3.12.1 -> 3.12.9, `libro` 2.8.2 -> 2.8.5, `majra` 2.5.1 -> 2.6.6.** All five were held
  back from the 2.0.1 cut so daimon would take one edit rather than two while ai-hwaccel 2.3.17 was
  still unpublished. The 2.0.1 release moved only `bote` 3.1.4 -> 3.3.1, which was forced (bayan's
  `bayan_json_v_parse_str` -> `_buf` rename left the old bote dist calling a symbol that no longer
  exists). Verified each bump took rather than merely built — every vendored `lib/<dep>.cyr` header
  confirmed at the new version. Suite **215/215**, unchanged.

## [2.0.1] - 2026-08-17

### Changed

- **Cyrius pin `6.4.69` -> `6.5.27`** (2026-08-17, ecosystem-wide ML/AI-arc realign ahead of
  the arc reopening). `cyrius lib sync --full` re-vendored the version-matched stdlib snapshot.
- **`[deps.bote]` `3.1.4` -> `3.3.1`** — required by the pin bump, and overdue on its own.
  bayan 1.3.0 (shipped in cyrius 6.5.0) renamed the cstr+len JSON entry
  `bayan_json_v_parse_str` -> `bayan_json_v_parse_buf`, so re-vendoring the 6.5.27 stdlib left
  bote 3.1.4's dist calling a symbol that no longer exists — one *reachable* undefined
  function, and `tests/daimon.tcyr` stopped compiling. bote fixed it upstream; 3.3.1 is the
  head of that line, and it also carries the two auth-bypass fixes called out in bote's own
  3.3.1 entry. Vendored `lib/bote.cyr` header confirmed moved to `# Version: 3.3.1`; suite
  back to **215/215**, the same count as before the bump.

### Known issues

- **`samay` 1.0.1's dist still calls the removed `json_v_parse_str`** (`src/json.cyr:33`
  upstream). It links today only because that call site is unreachable from daimon — the
  build reports it as a warning, not the hard error bote's reachable call produced. Anything
  that reaches samay's `Str`-taking JSON entry will fail to link until samay migrates to
  `bayan_json_v_parse_buf`. Not fixable from here ([[feedback_never_fix_in_materialized_libs]])
  — it needs a samay release, then a `[deps.samay]` tag bump.

## [2.0.0] - 2026-07-21

**Scheduler extracted to samay.** daimon's `src/scheduler.cyr` and `src/cron.cyr` were a
duplicate of the samay task-scheduler library (samay is literally samay's extraction of
this code). daimon 2.0.0 **deletes both** and consumes samay (`[deps.samay]`,
`dist/samay.cyr`) instead — the single source of truth for scheduling, now with real cron
expressions, ai-hwaccel-aware placement, deterministic tie-breaks, JSON snapshot/restore,
and a security-audited surface that daimon's local copy never had.

### Changed — BREAKING
- **`src/scheduler.cyr` + `src/cron.cyr` removed.** The scheduler API is now samay's:
  `task_scheduler_*` / `scheduled_task_*` / `node_capacity_*` / `ResourceReq` /
  `SchedulingDecision`. `src/api_sched.cyr` is rewired onto it — the HTTP `/v1/scheduler/*`
  endpoints are behaviour-compatible except task IDs are now UUIDs (was a sequential
  counter) and `POST /schedule` emits samay's `SchedulingDecision` JSON.
- **Cyrius pin `6.4.66` → `6.4.69`** (samay's `#derive(Serialize)` f64 codec requires it).
- `[deps]`: added `[deps.samay]` (1.0.1) + `[deps.ai-hwaccel]` (2.3.15) and the stdlib
  `atomic`. samay's `uuid_v4` was renamed to `samay_uuid_v4` upstream (samay 1.0.1) to
  resolve a last-def-wins collision with libro's incompatible `uuid_v4(buf)`.

### Removed
- daimon's duplicated scheduler/cron implementation and its local test copies. Scheduler
  correctness is now covered authoritatively by samay's 296-assertion suite; daimon keeps
  a `samay_integration` smoke test proving the wiring. Suite: **215 assertions**, green.

## [1.4.3] - 2026-07-17

### Security
- **Process-wide `SIGPIPE` guard installed at startup (`signal_ignore(SIGPIPE)` in `main`).** A
  flagsless socket write to a peer that closed mid-response raises `SIGPIPE`, whose default
  disposition **terminates the process** — an unauthenticated remote DoS (cyrius `6.4.51` CHANGELOG).
  Investigation of the 1.4.2 dep-bump review confirmed the **live HTTP path is already covered**:
  daimon delegates both serve modes to sandhi `1.9.0`, and `sandhi_server_run_opts` /
  `sandhi_server_run_async` each install `SIG_IGN` for `SIGPIPE` at the top of their accept loop
  (`_sandhi_server_ignore_sigpipe`, sandhi 1.6.6), so the acute HTTP DoS was **not** present. This
  adds a daimon-owned belt-and-braces layer as the first statement of `main`, which (a) uses the new
  `6.4.51` stdlib `signal_ignore` / `Signal` helper directly rather than a raw `rt_sigaction`, (b)
  covers daimon's **own** flagsless `SYS_WRITE`s to sockets that do not route through sandhi (the
  `src/ipc.cyr` control-socket path), and (c) keeps the guarantee independent of sandhi's internal
  guard placement. Idempotent, no allocator/args dependency, no-op on Windows/agnos. Build clean;
  240 unit assertions pass.

## [1.4.2] - 2026-07-17

**Toolchain + dependency refresh to the cyrius `6.4.66` matrix, and a project-wide
error-constant namespacing (`ERR_*` → `DAIMON_ERR_*`).** The dependency set moves to exactly the
versions bote `3.1.4` was cut and retested against (cyrius `6.4.66`, libro `2.8.2`, majra `2.5.1`,
sakshi `2.4.6`, sigil `3.12.1`), and every daimon error constant is prefixed to satisfy cyrlint's new
`lint_error_enum_namespace` rule. No daimon runtime source change was needed for the dep bumps; 240
unit assertions pass, `cyrlint` reports 0 warnings, and `cyrius build --check-lib-sync` is clean.

### Changed
- **cyrius pin `6.4.34` → `6.4.66`.** Carries, among many fixes: the aarch64 epoll reactor fix
  (`6.4.42` — the reactor had never worked on aarch64), close-on-exec for reactor fds across
  fork/exec (`6.4.43`), a `net.sock_accept` bump-heap leak fix (`6.4.61`), and folds the bundled
  sandhi to `1.9.0` + bayan to `1.2.0`. It is also the required floor (`≥ 6.4.65`) for
  `thread_local_alloc`, which libro `2.8.2`'s transitive sigil `3.12.1` / patra `1.12.12` call.
- **Dependencies bumped to bote `3.1.4`'s tested matrix:** `[deps.sakshi]` `2.4.4` → `2.4.6`,
  `[deps.libro]` `2.7.10` → `2.8.2`, `[deps.majra]` `2.5.0` → `2.5.1`, `[deps.bote]` `3.1.1` →
  `3.1.4`. No daimon `.cyr` change was required — every symbol daimon calls is unchanged across the
  range (libro `chain_new` / `chain_append` / `chain_append_with_agent` / `SEV_*`; bote
  `libro_tools_init` / `libro_tool_*` / `web_fetch_handler` / `web_search_handler`). libro and bote
  each namespaced their *own* `ERR_*` → `LIBRO_ERR_*` / `BOTE_ERR_*` upstream, which also removes
  latent collisions from daimon's flat single-pass link.
- **All daimon error constants namespaced `ERR_*` → `DAIMON_ERR_*`** (`src/error.cyr`'s `DaimonError`
  enum + every reference across `src/` and `tests/daimon.tcyr`). **Numeric values and the HTTP `code`
  response field are unchanged** — this is a source-symbol rename, not a wire-contract change. It
  satisfies cyrlint's new `lint_error_enum_namespace` rule (cyrius `6.4.51`: leaf projects must
  namespace their `ERR_*` set; the canonical unprefixed set is sakshi's) and structurally retires the
  earlier per-name workaround (`ERR_IPC_FAULT`, added at 1.3.0 to dodge majra's `ERR_IPC = 4`; now
  `DAIMON_ERR_IPC_FAULT`).
- **`sigil` moved from `[deps].stdlib` to an explicit `[deps.sigil]` git pin (`3.12.1`,
  `modules = ["dist/sigil.cyr"]`)**, mirroring bote/libro/majra. See **Fixed** below.

### Fixed
- **227 `duplicate fn (last definition wins)` build warnings eliminated.** sigil `3.12.x` ships a
  *modular* dist bundle (`dist/sigil-mldsa.cyr`, `dist/sigil-sha.cyr`, …) that libro `2.8.x`'s
  `.deps` sidecar pulls; this collided with the monolithic `dist/sigil.cyr` the stdlib snapshot
  vendored — two packagings of the *same* sigil release double-defining sha256 / ML-DSA / hex / … .
  The explicit `[deps.sigil]` pin at `modules = ["dist/sigil.cyr"]` (self-contained umbrella)
  overrides the transitive modular selection, so exactly one packaging is vendored. Build duplicate-fn
  warnings drop **229 → 2** (the remaining two are unrelated cross-bundle majra/bote overlaps).

## [1.4.1] - 2026-07-09

**Native HTTPS large responses fixed via a toolchain bump — no daimon source change.** The `web_fetch` /
`web_search` tools hosted in 1.4.0 call the sandhi client over its default **native** TLS backend, which had a
size-dependent record-layer bug in the stdlib: any response whose body arrived in a full 16 KB TLS record
failed (small pages like `example.com` slipped through). Fixed in the toolchain's stdlib TLS module and picked
up here by the pin bump.

### Changed
- **cyrius pin `6.4.20` → `6.4.34`** — carries the native TLS record-layer fix (decrypt-buffer off-by-one +
  `tls_native_read` partial-record delivery). `web_fetch` now works over the sovereign native backend against
  real hosts (anthropic.com / cyriusb.com / secureyeoman.ai / robertmaccracken.com), byte-identical to libssl.
- **`[deps.bote]` `3.1.0` → `3.1.1`** — bote re-cut onto the same toolchain (its web-tools source is unchanged;
  a local libssl-fallback workaround for this now-fixed root cause was dropped, never released).

## [1.4.0] - 2026-07-09

**Web tools hosted (`web_fetch` / `web_search`).** daimon now registers bote's new web tool family (bote
`3.1.0`) as built-in MCP tools, so an MCP client (thoth, via its agentic loop) can fetch pages and search the
web through the spine — t-ron-gated, no reimplementation in the consumer. The handlers live in bote; daimon
hosts + dispatches them (the same in-process builtin path as `libro_*`). Verified end-to-end: `web_fetch
http://example.com` returns clean readable text; `web_search` degrades honestly with no `BOTE_SEARXNG_URL`.

### Added
- **`mcp_web_init`** (`src/mcp_builtin.cyr`) registers `web_fetch` + `web_search`; `mcp_dispatch_builtin`
  routes them to bote's `web_fetch_handler` / `web_search_handler`; `app_init` calls it after `mcp_libro_init`.
- `[deps.bote]` bumped to `3.1.0` (the release carrying `src/web_tools.cyr`).

### Fixed
- `_mcp_wrap_builtin` no longer double-wraps an already-conformant result: a builtin that returns a full MCP
  content block (bote's `fs_*` / `web_*` tools) now passes through verbatim, while bare builtin JSON (libro's
  `{"ok":...}`) is still wrapped. Previously a web result came back as a content block nested inside another.

## [1.3.5] - 2026-07-07

**Built-in `libro_*` tools now return MCP-conformant results, plus a toolchain refresh to cyrius `6.4.20`.**
The five built-in audit tools (`libro_query` / `libro_verify` / `libro_export` / `libro_proof` /
`libro_retention`) dispatched in-process and returned their bare daimon JSON (`{"ok":true,...}` /
`{"ok":false,"error":...}`) straight out of `/v1/mcp/call` — which is NOT a conformant MCP `tools/call`
result. A strict MCP client that reads `content[0].text` (e.g. thoth's `/call` and its agentic tool loop)
found none and rendered "no text content could be parsed", so the tools appeared to return nothing.
Externally-hosted tools were unaffected (they already return conformant results and pass through verbatim).
240 unit assertions + the libro/tracing smokes pass on the new toolchain.

### Fixed
- **Built-in tool results are wrapped in an MCP content-block envelope** (`src/api_mcp.cyr`, new
  `_mcp_wrap_builtin`): `POST /v1/mcp/call` for a built-in tool now returns
  `{"content":[{"type":"text","text":"<raw result JSON>"}],"isError":<bool>}`, with `isError` derived from
  the result's `ok` field (`ok:false` → `isError:true`). The `libro_tool_*` functions (vendored
  `lib/bote.cyr`) are unchanged — the wrapping happens at the dispatch site, so the audit-chain output is
  preserved verbatim inside the text block. Verified live: `libro_query` → `isError:false`,
  `libro_retention` (missing arg) → `isError:true`.

### Changed
- **Toolchain pin `6.4.1 → 6.4.20`** (`cyrius.cyml` + `cyrius lib sync` — 64 floor modules; `cyrius deps`
  re-locked the 6 git deps). Clears the drift warning; the binary builds and the full suite passes.

### Release
- VERSION `1.3.4 → 1.3.5`; `cyrius.cyml` tracks it via `${file:VERSION}`. The zugot marketplace recipe
  pin (`marketplace/daimon.cyml`) and the git tag are the user's release steps.

## [1.3.4] - 2026-07-03

**Toolchain → cyrius `6.4.1` + sakshi `2.4.4`, which land the two upstream fixes
daimon was waiting on — the VULN-007 structural fix and 128-bit trace-ids — so
the 1.3.2 gate and both 1.3.3 tracing limitations close.**

### Changed

- **cyrius pin `6.3.43` → `6.4.1`** (`cyrius.cyml`); **`[deps.sakshi]` `2.4.3` →
  `2.4.4`**. `cyrius lib sync` + `cyrius deps` re-vendored (`cyrius.lock` 72
  deps). No daimon source breakage from the 6.4.0 `CYRIUS_MONOMORPH` default-on
  flip (daimon uses no generics; it was decoupled from general inlining +
  verified byte-identical upstream). 240/240 tests.
- **Full 128-bit trace-id adoption** (`src/trace.cyr`). Now that sakshi 2.4.4
  ships `sakshi_trace_set_128` / `_hi` / `_lo`, daimon adopts the **whole** W3C
  `traceparent` trace-id (high 64 bits at offset 3, low at 19) instead of
  folding to the low half; `X-Trace-Id` accepts 32-hex (128-bit) or 16-hex
  (64-bit). The `X-Trace-Id` response echo is now the full 32-hex id. Closes the
  1.3.3 "i64-only" limitation. (+3 test assertions; 237 → 240.)
- **Outbound trace-context propagation** (`src/trace.cyr`, `api_mcp.cyr`). The
  external MCP forward now injects a W3C `traceparent` header (active trace id +
  a fresh span id) via `sandhi_rpc_mcp_call_with_headers`, so the downstream
  endpoint joins the trace. Closes the 1.3.3 "no outbound propagation"
  limitation. (The header API already existed in sandhi 1.7.0 — the 1.3.3 note
  had misread the API; no sandhi change was needed.)

### Security

- **VULN-007 structural half — CLOSED upstream.** cyrius 6.4.1 landed
  zero-on-reset in `alloc_reset()` across all four allocator backends (the
  `_alloc_zero` scrub before the bump-pointer rewind), which daimon now vendors.
  The bump-allocator reuse channel is closed at the source. daimon's 1.3.2
  consumer-side secret-hygiene stands as defense-in-depth; **per-agent arena
  isolation remains the only open half**, still gated for multi-tenant.

### Verified

- `cyrius build`: OK. `cyrius tests`: **240 / 240**. `cyrius fmt --check` +
  `cyrius lint`: clean. `test.sh`: libro smoke 6/6, tracing smoke passes now
  asserting the **full 128-bit** `traceparent` round-trips through `X-Trace-Id`,
  5/5 fuzz. Vendored `alloc.cyr` confirmed carrying the zero-on-reset scrub.

## [1.3.3] - 2026-07-03

**Distributed tracing over sakshi.** daimon now participates in a distributed
trace: it adopts an inbound trace id, correlates its sakshi spans under it,
spans key operations, and echoes the id back so a caller can stitch daimon's
work into its own trace. Opt-in (`serve --trace`); off by default and a no-op
when off.

### Added

- **`src/trace.cyr`** — trace context over sakshi's span stack + i64 trace id.
  - **Inbound adoption**: at the request boundary, extract the trace id from a
    W3C `traceparent` (the trace-id's low 64 bits) or `X-Trace-Id` header, else
    generate a random one (`random_bytes`). Set via `sakshi_trace_set`.
  - **Spans**: a root `http.request` span per request (single balanced
    enter/exit around routing) + nested `mcp.builtin` / `mcp.forward` spans
    around MCP dispatch. Timed and emitted by sakshi.
  - **Outbound echo**: `X-Trace-Id: <16 hex>` on every response (added centrally
    in `http_send_response` via sandhi's `extra_headers` slot).
  - `--trace` flag on the `serve` command toggles it; when off, every helper is
    a cheap guard-and-return and no header is emitted.
  - New `trace_id` test group (+5 assertions; 232 → 237: hex parse/format
    round-trip, traceparent low-64 extraction, non-hex stop) and a `test.sh`
    integration smoke (traceparent adopted → echoed). `trace_id_hex` benchmark
    (~1.4 µs).

### Notes

- **Limits (honest).** sakshi holds a single **i64** trace id, so a W3C 128-bit
  trace-id is folded to its low 64 bits — correlation within a daimon-rooted
  trace is exact, but a full 128-bit round-trip is not preserved. And
  `sandhi_rpc_mcp_call` takes **no custom request headers**, so daimon cannot
  inject `traceparent` on external MCP forwards — the `mcp.forward` span is
  local-only until sandhi's rpc gains header support.

### Verified

- `cyrius build`: OK. `cyrius tests`: **237 / 237**. `cyrius fmt --check` +
  `cyrius lint`: clean. `test.sh` tracing smoke passes (W3C `traceparent`
  adopted and echoed as `X-Trace-Id`); span emission verified live.

## [1.3.2] - 2026-07-03

**VULN-007 (bump-allocator memory reuse) — consumer-side secret-hygiene
mitigation, plus the structural fix filed upstream.** Investigation confirmed
the reuse channel lives entirely in vendored stdlib (the cyrius bump allocator's
`alloc_reset()` rewinds without zeroing, and it is *sandhi* — not daimon — that
resets between request batches). daimon's own global allocator is never reset
during operation, so daimon-allocated data is never handed to a new owner via
address reuse. What daimon *can* own is secret hygiene: scrubbing sensitive
buffers so they don't linger in the never-freed bump heap (core-dump /
`/proc/<pid>/mem` exposure). Full multi-tenant isolation remains gated on the
upstream fix + per-agent arenas.

### Added

- **`secure_zero(ptr, len)` / `secure_zero_str(s)`** (`src/secmem.cyr`) —
  DSE-resistant zeroing of sensitive bytes at end-of-life. Cyrius has no
  `volatile` keyword and does no cross-function dead-store elimination, so the
  scrub gets its resistance from function-boundary opacity plus a sink read-back
  (belt-and-suspenders / future-proofing). ~6.7 µs to scrub 4 KiB
  (`secure_zero_4k` benchmark). New `secure_zero` test group (+7 assertions;
  225 → 232).

### Changed

- **Memory store scrubs its transient value buffers** (`src/memory.cyr`).
  `memory_store_list_by_tag` and `memory_store_usage_bytes` each read an agent's
  full memory value into a heap buffer, examine it (tag match / byte count), and
  discard it — leaving the value resident in the never-freed heap. Both now
  `secure_zero` the buffer once the value is consumed. Negligible cost (one pass
  over already-file-read bytes).

### Security

- **VULN-007 upstream fix filed** —
  [`docs/development/issues/archive/2026-07-03-cyrius-alloc-reset-no-zero-reused-memory.md`](docs/development/issues/archive/2026-07-03-cyrius-alloc-reset-no-zero-reused-memory.md)
  (mirrored to the cyrius repo). `alloc_reset()` rewinds the bump pointer to the
  first chunk without zeroing the reclaimed span, so reset-then-reallocate reuses
  addresses holding the prior occupant's bytes (CVE-2026-34988 / Wasmtime class).
  daimon can't fix it (vendored stdlib; the reset is driven by sandhi); proposed
  fix is zero-on-reset (`memset` the used span, or `MADV_DONTNEED`).
- **Scope is honest defense-in-depth, not a complete VULN-007 fix.** It does not
  close the reuse channel (upstream) and does not provide multi-tenant isolation
  (still requires per-agent arenas — gated). It reduces how long sensitive agent
  data is resident in the heap. IPC message payloads (fanned out to multiple
  subscriber queues) and external MCP response data (sandhi-owned) are
  deliberately NOT scrubbed — doing so would corrupt live/shared memory.

### Verified

- `cyrius build`: OK. `cyrius tests`: **232 / 232**. `cyrius fmt --check` +
  `cyrius lint`: clean. `secure_zero_4k` benchmark added.

## [1.3.1] - 2026-07-03

**Documentation sweep.** Roadmap trimmed to open-work-only; README expanded; the
doc surface reconciled to the 1.3.x state.

### Changed

- **`docs/development/roadmap.md` trimmed to open work only** — all completed
  (`[x]`) sections removed (that history is the CHANGELOG's job) and the met
  v1.0-criteria block dropped; consolidated to the VULN-007 security gate, the
  nein firewall-MCP upstream blocker, and the v1.4.0+ backlog under a lean status
  header.
- **`README.md` expanded** — intro notes the libro audit trail; deps example
  lists sakshi/bote/libro/majra; benchmark count corrected; added an "MCP audit
  tools" section and a "Documentation" link block.
- **`SECURITY.md`** supported-versions rolled `1.0.x` → `1.3.x` (+ `< 1.3`
  unsupported). **`docs/guides/api.md`** documents the built-in libro tools
  alongside the external-tool examples. **`docs/doc-health.md`** ledger rolled to
  the 1.3.1 sweep.

## [1.3.0] - 2026-07-03

**Toolchain to latest (`6.2.11` → `6.3.43`), VERSION established as the single
source of truth for daimon's version, the doc surface refreshed to the current
pins, and bote's libro audit tools wired into the MCP host as builtin,
in-process-dispatched tools.**

### Added

- **bote libro audit tools hosted as builtin MCP tools** (`src/mcp_builtin.cyr`).
  daimon owns a single hash-linked libro audit chain (`chain_new`, seeded with a
  genesis entry at startup) and exposes bote's five `libro_*` tools —
  `libro_query` / `libro_verify` / `libro_export` / `libro_proof` /
  `libro_retention` — as builtins in its own MCP registry. `POST /v1/mcp/call`
  now dispatches builtin tools **in-process** (previously **501 Not
  Implemented**): it extracts the raw MCP `arguments` object and calls bote's
  handler, returning the handler JSON verbatim (bote's own `resp_success(id,
  result)` convention). `/v1/mcp/tools` advertises the five tools alongside
  daimon's existing builtin + external tools. `mcp_dispatch_builtin` returns 0
  for an unregistered builtin, preserving the 501 path for tool kinds with no
  in-process handler.
- **daimon's own audit events feed the libro chain** (`src/audit.cyr`). The
  chain owned by `audit.cyr` (genesis-seeded at `daimon_audit_init`) is the same
  one the libro_* tools read, so `query`/`verify`/`export`/`proof`/`retention`
  operate on a live audit trail. Instrumented events: `agent.spawn` /
  `agent.stop` (INFO, agent-scoped), `ipc.auth.deny` (SECURITY — SO_PEERCRED UID
  mismatch), `http.ratelimit` (WARNING — per-IP 429), `mcp.register.reject`
  (SECURITY — SSRF-guard rejection), and `mcp.call` (INFO — external tool
  forward). `daimon_audit*` is a no-op until the chain is up, so instrumented
  paths are safe in unit contexts. Security-sensitive/attacker-controlled data
  is kept out of the entry `details` field (libro's tool JSON emits
  severity/source/action/agent_id, not details).
- **Dependencies added** for the above: `bote 3.0.0` (the `dist/bote.cyr`
  bundle), its transitive `libro 2.7.10` (audit chain) and `majra 2.5.0` (event
  sink, present for compile-time resolution only), plus the stdlib modules they
  require (`ct`, `keccak`, `random`, `slice`, `thread_local`, `sync`,
  `ws_server`). `cyrius.lock` grew to 71 deps.
- **Integration smoke** in `tests/test.sh` — starts the server and asserts the
  manifest advertises the five tools and that `libro_export` / `libro_verify` /
  `libro_query` dispatch correctly over the seeded chain (the `.tcyr` unit suite
  is self-contained and can't reach the linked bundle code).

### Changed

- **`DaimonError.ERR_IPC` (=5) renamed `ERR_IPC_FAULT` (=10)** across `error.cyr`
  + the 13 `ipc.cyr` call sites. majra (via the bote bundle) defines its own
  `ERR_IPC = 4`; cyrius's constant-collision guardrail flags a same-name /
  different-value redefinition. Same class as the `ERR_IO` note in 1.2.9.

- **cyrius pin `6.2.11` → `6.3.43`** (`cyrius.cyml`) — latest toolchain; drift
  between the pin and the installed wrapper is cleared (`cyrius --version` now
  reports `manifest-pin: 6.3.43`, no drift warning). Vendored stdlib resynced
  from the 6.3.43 snapshot: **sandhi `1.6.2` → `1.7.0`**, **sigil `3.7.13` →
  `3.10.0`**. `sakshi` git-pin `2.3.0` → `2.4.3`. `cyrius.lock` re-resolved to
  61 deps.
- **`sigil` added to `[deps].stdlib`.** 6.3.43's `tls_native_lowlevel` (pulled
  in transitively by sandhi's bundle for compile-time symbol resolution) now
  references `sha384_init_into`, which is defined in `sigil`. `cyrius lib sync`
  vendors the *declared* stdlib subset, so sigil was left at its stale 3.7.13
  copy (lacking the symbol) → a reachable-undefined build error until sigil was
  declared. This is the same transitive-closure requirement already documented
  for `tls`/`mmap`/`dynlib`/`fdlopen`; sigil is the next such entry. daimon
  calls nothing in sigil directly — present for compile-time resolution only;
  the crypto path is unreachable at runtime.
- **VERSION is the single source of truth for daimon's version.** New
  `daimon_version()` helper (`config.cyr`) is the only code path that produces
  the version string, reading the `VERSION` file (the same file `cyrius.cyml`
  reads via `${file:VERSION}` for `[package].version`). The two duplicated
  read-`VERSION`-then-fall-back sites (`main.cyr` `version` command,
  `server.cyr` banner) collapse to one call each. The stale, duplicated
  `"1.1.0"` fallback literal is replaced by a single `"unknown"` sentinel —
  deliberately not a real number, so a missing VERSION file can never
  masquerade as a stale release. No daimon-version literal remains anywhere but
  `VERSION`.
- Documentation refreshed to the 6.3.43 / sandhi 1.7.0 / sakshi 2.4.3 / sigil
  3.10.0 pins (quickstart, architecture overview, README, CONTRIBUTING,
  guides/api, doc-health ledger); the v1.2.x doc-refresh backlog drained.

### Verified

- `cyrius build`: OK. `cyrius tests`: **225 / 225** pass. `cyrius fmt --check`
  + `cyrius lint`: clean. `daimon version` and the serve banner both render the
  VERSION-sourced string live. The `tests/test.sh` libro integration smoke
  passes (manifest lists 5 tools; `libro_export`/`verify`/`query` dispatch over
  the seeded chain) and all 5 fuzz harnesses build + pass. Two cross-bundle
  duplicate-fn warnings (`_sub_new`, `cancel_token_new` — majra/bote both bundle
  shared code; "last wins", harmless).

## [1.2.9] - 2026-06-15

**Toolchain + stdlib refresh: cyrius `6.2.2` → `6.2.11`, vendored sandhi
`1.4.10` → `1.6.2`.** Routine dependency bump that also clears a latent
cross-module constant collision newly surfaced by 6.2.11's guardrail.

### Changed

- **cyrius pin `6.2.2` → `6.2.11`** (`cyrius.cyml`). Picks up the
  constant-collision guardrail (warns on data symbols redefined with a
  conflicting compile-time value), the aarch64-Linux INET/`poll`→`ppoll`
  syscall fix, and the Darwin IPv6 socket surface.
- **Vendored stdlib resynced to the 6.2.11 snapshot** — `lib/sandhi.cyr`
  `1.4.10` → `1.6.2` (composes the new `net` IPv6 + non-blocking surface,
  drops its hand-rolled Darwin socket shims) and the `tls_native` module
  split into per-concern peers (`tls_native_conn/ctx/hs12/hs13/keysched/
  lowlevel`). `cyrius.lock`'s reachable closure grew 52 → 60 entries
  accordingly (gitignored `lib/` repopulated by `cyrius deps`). `sakshi`
  stays at `2.3.0` (already latest).

### Fixed

- **`DaimonError.ERR_IO` removed — name-collided with sigil's
  `SigilError.ERR_IO`.** sigil (pulled in transitively via `tls`) defines
  `ERR_IO = 6` and returns it from four internal I/O-failure paths; daimon's
  enum defined `ERR_IO = 10`. Under cyrius's flatten-to-one-scope model this
  is a "last definition wins" collision (newly flagged by the 6.2.11
  guardrail) that could silently corrupt sigil's error code. daimon never
  referenced `ERR_IO`, so the slot was dropped; reintroduce only under a
  non-colliding name (e.g. `ERR_IO_FAULT`).

## [1.2.8] - 2026-06-12

**Codebase refactor — the 4.1k-line `src/main.cyr` monolith split into 25
per-domain modules.** Pure structural change: behavior-preserving, no API / wire
/ runtime change.

### Changed

- **`src/main.cyr` (4,132 LOC) → 26 files**, none over ~350 LOC. `main.cyr` is now
  a 132-line composition entry point (syscall-constant preamble + module
  `include`s + the `main` serve loop). The domain modules: `error`, `config`,
  `agent`, `supervisor`, `memory`, `vector_store`, `rag`, `mcp`, `screen`,
  `scheduler` + `cron` (interval triggers split out), `federation` +
  `fed_vector_store` (replica merge/dedup split out), `edge`, `ipc`, and the HTTP
  layer broken into `app` (global state + `app_init` composition root), `http`
  (constants, JSON escaping, request/response helpers), the route handlers grouped
  by domain (`api` = health/metrics, `api_agent`, `api_mcp`, `api_rag`,
  `api_edge`, `api_sched`), `router` (`http_route` dispatch), and `server` (rate
  limiting, per-request dispatch, sync/async serve). Largest file is now
  `scheduler.cyr` / `agent.cyr` / `ipc.cyr` at ~345 LOC (were 421 / 345 / 341).
- **Behavior-preserving by construction.** cyrius flattens `include`s into one
  global scope. Contiguous splits (most modules, the federation and cron splits)
  are sliced at existing section banners in original source order → the
  preprocessed token stream is **byte-identical** (verified by `md5sum` /
  `diff`). The HTTP route handlers were regrouped by domain (pure functions, no
  top-level state — verified `grep`), so order changed but the **sorted set of
  code lines is identical** to the original `api.cyr` (verified by `diff`), and
  every endpoint was exercised live.
- CI needs no change — its fmt/lint/security steps already glob `src/*.cyr`, and
  the build entry (`src/main.cyr`) pulls in every module. `docs/architecture/
  overview.md` module map updated to the multi-file layout.

### Verified

- `cyrius build`: OK. `cyrius test`: **225 / 225** pass. `cyrius fmt --check` +
  `cyrius lint`: clean across all 26 `src/*.cyr`. Live smoke across every endpoint
  domain (health, metrics, mcp, edge, scheduler, rag) after the handler regroup.
  Benchmarks unchanged (byte-identical binary behavior).

## [1.2.7] - 2026-06-12

**MCP per-tool input schemas + cyrius 6.2.2 toolchain / element-typed-array
adoption.** Closes the last gap blocking high-fidelity model-driven tool calling
(filed by thoth,
`docs/development/issues/archive/2026-06-11-mcp-manifest-omits-tool-input-schema.md`)
and adopts cyrius 6.2.1's language-level fix for the 1.2.6 fixed-local-array
footgun.

### Added

- **`inputSchema` on MCP tool registration + manifest.** `POST /v1/mcp/tools`
  now reads an optional `inputSchema` (accepting the `input_schema` snake_case
  alias) — a nested JSON Schema object — and stores it verbatim on the tool;
  `GET /v1/mcp/tools` emits it as raw JSON per tool. The `McpToolDescription`
  struct already carried the slot (`input_schema`, accessor `mcp_tool_schema`);
  it was write-once-empty (`mcp_tool_new(name, desc, str_from("{}"))`) and never
  exported. Consumers (thoth → hoosh) can now pass real `function.parameters`
  schemas to the model instead of a permissive `{"type":"object"}` guess.
  - **Nested-object extraction.** The flat `json_parse`/`jget` path scans a value
    to the first `,`/`}` and so mangles nested objects; `mcp_extract_input_schema`
    instead uses bayan's typed engine (`bayan_json_v_parse` →
    `bayan_json_v_obj_get` → `bayan_json_v_build`), requiring a JSON object and
    re-emitting it compactly into fresh heap (not aliased to the request buffer —
    consistent with the 1.2.5 deep-copy fix). ~1 µs per registration body
    (`mcp_extract_input_schema` bench).
  - **Manifest fallback.** `mcp_manifest_schema` emits the stored schema verbatim,
    falling back to a permissive `{"type":"object"}` for any tool with no schema
    (builtins register none today) so the emitted `inputSchema` is always valid
    JSON.
  - **Backward-compatible.** Absent `inputSchema` still defaults to `{}`; the
    change is an additive response field + a new optional request field. No
    route/contract change.

### Fixed

- **Daimon-class slot-array sweep — every address-taken fixed local array sized
  to its real footprint** (`src/main.cyr`). cyrius 6.2.1 gave bare `var a[N]`
  explicit per-scope semantics (**N bytes in a function**, N i64 slots only at
  top level) and added element-typed `var a: T[N]` (full `N * sizeof(T)` bytes in
  any scope). Under that rule daimon's address-taken slot/byte arrays were
  under-reserved (a function-local `var a[4]` is 4 bytes, not 32) — latent,
  layout-sensitive corruption on exec / IPC / edge-status paths (the 1.2.6
  `parts[4]` routing-404 bug was the first symptom). Converted:
  - `argv_buf: i64[4]` (execve argv — 4 pointer slots), and the two edge/scheduler
    `status_names: i64[5]` / `i64[7]` status tables (store64 string slots).
  - Syscall scratch buffers to exact widths: `status_buf: i32[1]` (wait4 status
    int), `cred_len: u32[1]` (SO_PEERCRED socklen_t), `len_buf: u8[4]` +
    `hdr: u8[4]` (4-byte big-endian IPC length prefixes). The genuine 1-byte
    buffers (`ack_buf`, two `cbuf`) stay bare `[1]` — now idiomatically 1 byte.
  - `ip_to_cstr` keeps its inline-octet form (allocates no array; clearest for
    the rate-limiter hot path); stale "compiler-bug workaround" comment updated.

### Changed

- **`cyrius.cyml` pin `6.1.40` → `6.2.2`** (frontend annotation-token fix for
  aarch64/macOS + ecosystem stdlib fold-in; `cyrius.lock` 52 deps). The 6.2.1
  fix was a **language change**, not a silent codegen patch — see the cyrius
  6.2.1 CHANGELOG ("element-typed arrays + daimon-class slot-array sweep").
- **Sakshi pin `2.2.10` → `2.3.0`** — the re-folded release carrying sakshi's own
  daimon-class fix (`ts[2]` timespec hot path); requires pin ≥ 6.2.1.

### Verified

- `cyrius lint`: 0 warnings (src + tests + bench + fuzz). `cyrius fmt --check`:
  clean. `cyrius test`: **225 / 225** pass (+8). Build clean (pin matches cycc
  6.2.2). Live: register with a nested `inputSchema` → manifest round-trips it as
  raw JSON, register without → `"inputSchema":{}`; edge-node GET renders
  `"status":"Online"` (exercises `status_names: i64[5]`); element-typed
  `var parts: i64[4]` reproducer returns exit 32 (no corruption) under 6.2.2,
  confirming the upstream fix.
- Resolved compiler-bug issue archived
  (`docs/development/issues/archive/2026-06-11-cyrius-addr-taken-local-array-static-overlap.md`).

## [1.2.6] - 2026-06-11

**Async server collapse + aarch64 unblock + a HIGH-severity compiler-bug
workaround uncovered along the way.** Toolchain pin moves **6.1.39 → 6.1.40**.

### Fixed

- **HIGH — every HTTP route silently 404'd in certain build layouts (static
  memory corruption).** Root cause is a **cyrius compiler bug**: an address-taken
  fixed local array `var a[N]` is placed in static storage but under-reserved by
  one slot (`(N-1)*8` bytes), so a write to its last element corrupts the
  adjacent static object. daimon's `ip_to_cstr` wrote `store64(&parts + 24, octet)`
  on `var parts[4]`; for a `127.0.0.1` peer the `.1` octet landed on sandhi's
  single-space `" "` string literal. `sandhi_server_get_path` then searched the
  request line for byte `1` instead of `0x20`, found no space, returned an empty
  path → **all `/v1/*` routes 404'd, sync and async**. Latent and layout-sensitive
  — it surfaced only because the serve_async refactor below shifted static layout
  so `parts` landed 24 bytes before that literal.
  - **Workaround (this release):** `ip_to_cstr` now computes each octet inline
    (`(ip >> (pi*8)) & 255`) instead of staging through an address-taken
    `var parts[4]` — no `&`-of-local-array, no static placement, bug avoided.
  - **Root fix is upstream (cyrius).** Reported with a ~20-line standalone
    reproducer in `docs/development/issues/archive/2026-06-11-cyrius-addr-taken-local-array-static-overlap.md`.

### Changed

- **Collapsed `serve_async` onto `sandhi_server_run_async`.** The hand-rolled
  epoll accept loop (1.1.0–1.2.5) is replaced by sandhi's epoll-cooperative loop,
  unblocked upstream by sandhi 1.4.9's `max_conns` enforcement. Wins over the old
  loop: a **bounded, reused per-batch arena** for recv buffers (the old path
  alloc'd 64 KiB/connection from the never-freeing global bump — leaked every
  batch) and an **enforced concurrency cap** (`SERVE_MAX_CONNS = 128`, was an
  implicit `max_batch = 64`). Per-connection `SO_RCVTIMEO` and CL/TE +
  duplicate-header smuggling rejection now come from sandhi on both paths; the
  shared `handle_request(ctx, cfd, buf, len)` contract is unchanged. Removed the
  now-dead `server_bind` / `set_recv_timeout_ms` / `async_handle_client` helpers
  and the `SYS_SETSOCKOPT` constant. Verified: sync + async both serve correctly;
  100 concurrent async requests all return 200.
- **aarch64 cross-build unblocked** — `agent_spawn_with_limits` now calls
  `sys_fork()` instead of raw `syscall(SYS_FORK)`. x86_64 wraps `SYS_FORK`;
  aarch64 (no `fork(2)`) wraps `clone(SIGCHLD)` via `SYS_CLONE`, the shim cyrius
  6.1.x ships. This was the last aarch64-absent syscall after the epoll gap was
  fixed upstream at 6.1.24.
- **`cyrius.cyml`** — `cyrius = "6.1.40"` (was `"6.1.39"`).

### Verified

- `cyrius lint`: 0 warnings. `cyrius fmt --check`: clean. `cyrius test`:
  **217 / 217** pass. `cyrius build`: OK; sync + async routing verified live
  (`/v1/health`, `/v1/metrics`, `POST /v1/mcp/tools`), 100/100 concurrent async.

## [1.2.5] - 2026-06-11

**Security fix + toolchain bump.** Closes a HIGH-severity registry-aliasing bug
reported by thoth (consumer), and moves the language pin **6.1.24 → 6.1.39**.

### Fixed

- **MCP host registry aliased the transient request buffer (HIGH).** `mcp_register_external`
  stored the `name` / `description` / `callback_url` `Str`s produced by `json_parse(body)`
  directly into the registry. Those `Str`s point into the per-connection request buffer,
  which sync mode reuses for later requests — so every subsequent request overwrote the
  bytes the registry pointed at. Symptoms: `GET /v1/mcp/tools` rendered tool names/descriptions
  as fragments of *later* requests' bytes, and `POST /v1/mcp/call` returned `502 {"upstream":""}`
  because the stored `callback_url` was clobbered. Worse than availability: a clobbered URL is
  *whatever bytes a later request left at that offset*, which could steer an external tool call
  to an attacker-influenced URL **after** registration, sidestepping the `validate_callback_url`
  SSRF guard that runs only at registration time.
  - **Fix:** `mcp_register_external` now deep-copies (`str_clone`) the tool's `name` /
    `description` / `schema` and the `callback_url` into registry-owned (bump-allocated,
    process-lifetime-stable) storage before storing. `api_mcp_call` re-validates the stored
    URL with `validate_callback_url` at dispatch time (defense-in-depth; returns `502` if a
    stored URL is ever non-http(s)).
  - **Verified:** new `mcp_registry` regression test registers an external tool from a mutable
    buffer, overwrites every source byte (as a later request would), and asserts the registry
    still resolves the original name + URL. Live repro from the issue now passes — a tool
    registered with a 1-byte description survives arbitrary intervening requests.
  - Reported in `docs/development/issues/archive/2026-06-11-mcp-registry-aliases-request-buffer.md`
    (thoth 0.3.0, M4). Consumer-side padding workaround is no longer needed.

### Changed

- **`cyrius.cyml`** — `cyrius = "6.1.39"` (was `"6.1.24"`). The `json` stdlib module was folded
  into the **`bayan`** distribution bundle in the 6.1.x line (`json_parse` / `json_get` /
  `json_get_int` ship as compat shims from `bayan.cyr`); `[deps].stdlib` swaps `"json"` → `"bayan"`,
  and the two test files' `include "lib/json.cyr"` become `include "lib/bayan.cyr"`. Daimon's own
  `json_escape_str` (src/main.cyr) is unaffected. `cyrius.lock` re-resolved: **52 deps locked**.

### Verified

- `cyrius lint src/main.cyr`: 0 warnings. `cyrius fmt --check`: clean. `cyrius test`:
  **217 / 217** assertions pass (was 213 — +4 from the registry regression test).
- `cyrius build src/main.cyr`: OK. Binary boots clean: `daimon v1.2.5 listening on port 8090 (sync)`.

## [1.2.4] - 2026-06-10

**Major toolchain bump + dependency refresh.** Moves the language pin from cyrius **5.10.44 → 6.1.24** (first 6.x pin) in `cyrius.cyml`, bumps the `sakshi` git-pin **2.2.3 → 2.2.10**, and picks up the bundled **sandhi 1.3.3 → 1.4.10** that rides in with the 6.1.24 toolchain. No daimon source changes — the sandhi/sakshi call surfaces daimon uses are unchanged.

### Changed

- **`cyrius.cyml`** — `cyrius = "6.1.24"` (was `"5.10.44"`); `[deps.sakshi].tag = "2.2.10"` (was `"2.2.3"`).
- **`cyrius.lock`** — re-resolved under 6.1.24; **53 deps locked** (was 37). The version-pinned stdlib snapshot grew (new modules: `bigint`, `ct`, `keccak`, `sigil`, `thread_local`, `tls_native`, `*_agnos`/`*_win` arch variants).
- **`./lib/` snapshot refreshed** — the gitignored local snapshot carried stale 5.10.44 stdlib (sandhi 1.3.3) that shadowed the 6.1.24 version-pinned lib. Deleted and re-resolved via `cyrius deps` so the snapshot tracks the new toolchain (sandhi now 1.4.10).
- **`CLAUDE.md`** — project-identity line `Cyrius: 6.1.24`; sandhi-stdlib table cell + transitive-deps row `sandhi 1.3.3 → 1.4.10` / `5.10.44 → 6.1.24`; sakshi row `(2.2.3) → (2.2.10)`.

### Fixed (CI/release)

- **Toolchain install migrated to the upstream `install.sh`** (`.github/workflows/{ci,release}.yml`). The 6.x toolchain renamed the compiler binary `cc5` → `cycc`, so the pre-6.x hand-rolled two-tarball extraction (release tarball for `bin/`, source archive for `lib/`) failed its sanity gate with `FAIL: bin/cc5 missing — release tarball extraction failed`. Both workflows now pipe `https://raw.githubusercontent.com/MacCracken/cyrius/main/scripts/install.sh` with `CYRIUS_VERSION` read from the `cyrius.cyml` pin, matching bote / patra / agnosys on the 6.x toolchain. The redundant "Clean toolchain artifacts" release step (which `rm`'d the no-longer-downloaded `cyrius-*.tar.gz`) was dropped.
- **`cc5 --version` verify → `cyrius --version`** in both workflows (the `cyrius` wrapper is the stable entrypoint across the rename).
- **aarch64 cross-build lane updated for 6.x** — `cc5_aarch64` → `cycc_aarch64` (binary rename), and the known-upstream-blocker grep generalized from `SYS_EPOLL_WAIT` to `SYS_(FORK|EPOLL_WAIT)`. The async-epoll gap was fixed upstream by 6.1.24, so the next aarch64-absent syscall — `SYS_FORK` (aarch64 Linux is clone-only, no `fork(2)`) — is now the surfaced blocker. The lane stays best-effort warn-and-continue; the x86_64 build remains authoritative.

### Changed (docs, folded from Unreleased)

- **`docs/development/roadmap.md`** — refreshed two stale upstream version references after the 2026-05-11 first-party-set bump: (1) "Blocked on Upstream Ports" line for MCP tool hosting bumped `2.7.1 (as of 2026-05-10)` → `2.7.2 (as of 2026-05-11)` and adds a note that the `dist/bote-core.cyr` opt-in transport-free profile landed in bote 2.7.2 (t-ron 2.1.3 is the trigger consumer); (2) `README.md` footprint refresh todo bumped `cyrius 5.10.34` → `cyrius 5.10.44`.

### Verified

- **`TLS_EARLY_DATA_ACCEPTED` rationale still holds** — sandhi 1.4.10's bundle still references it (3 sites), so the transitive `tls`/`mmap`/`dynlib`/`fdlopen` stdlib deps (present since 1.2.0 for compile-time symbol resolution) stay required.
- **Daimon's sandhi API surface intact in 1.4.10** — `sandhi_server_run`, `sandhi_server_run_opts`, `sandhi_server_options_idle_ms`, `sandhi_server_recv_request`, `sandhi_rpc_mcp_call` all present.
- `cyrius lint src/main.cyr`: 0 warnings. `cyrius test`: **213 / 213** assertions pass (no test additions — a toolchain/dep bump touches no daimon source).
- `cyrius build` (DCE): **1,466,136 bytes** (~1.43 MB) statically-linked, stripped ELF. **Up from ~624 KB at 1.2.2** — this is a 6.x codegen change, not a daimon regression: 6.1.24's DCE NOPs the 1,797 unreachable fns (511,548 bytes) in place but keeps them in the section layout, where the 5.x toolchain stripped them. Binary boots clean: `daimon v1.2.4 listening on port 8090 (sync)`.

### Known issues

- **`serve_async` collapse — upstream resolution path now exists (1.4.9+), still a deferred follow-up.** Bundled sandhi 1.4.9 introduced `sandhi_server_run_async`, an epoll-cooperative accept loop that **does honor `sandhi_server_options_max_conns`** (the sync `sandhi_server_run` / `sandhi_server_run_opts` stay single-flight by design). This unblocks the long-deferred "collapse `serve_async` into a sandhi-driven accept loop" item — but the migration is a behavioural change (daimon's hand-rolled async epoll loop → `sandhi_server_run_async`) that earns its own work-loop cycle with tests + benchmarks, out of scope for this toolchain bump. Roadmapped.

## [1.2.3] - 2026-05-11

**Cyrius toolchain pin bump.** Moves the language pin from 5.10.34 → 5.10.44 in `cyrius.cyml` (and the mirrored line in `CLAUDE.md`). No daimon source changes; bundled sandhi version unchanged.

### Changed

- **`cyrius.cyml`** — `cyrius = "5.10.44"` (was `"5.10.34"`).
- **`CLAUDE.md`** — Project-identity line synced to `Cyrius: 5.10.44`.

## [1.2.2] - 2026-05-10

**Slowloris bound, on both sync and async.** Lowers the per-connection idle timeout from sandhi's 30 000 ms default to **5 000 ms** on the sync path (via `sandhi_server_run_opts` + `sandhi_server_options_idle_ms`), and **closes a pre-existing async-path slowloris gap** by applying `SO_RCVTIMEO` to async-accepted cfds (async had no per-connection timeout since 1.1.0). Second of the two 1.1.5 sandhi follow-ups; the third (`serve_async` collapse into `sandhi_server_run_opts`) stays deferred — upstream sandhi's `max_conns` is still accepted-but-not-honored, see Known issues.

### Added

- **`SERVE_IDLE_MS = 5000`** — centralized idle-timeout constant in `src/main.cyr`. Used by both `serve` (sync, via sandhi opts) and `serve_async` (async, via direct `SO_RCVTIMEO` syscall). One source of truth; keeps both paths in lock-step.
- **`set_recv_timeout_ms(fd, ms)`** — daimon-side helper that applies `SO_RCVTIMEO` via `syscall(SYS_SETSOCKOPT, fd, SOL_SOCKET=1, SO_RCVTIMEO=20, &tv, 16)`. `tv` is a 16-byte `struct timeval` `{tv_sec, tv_usec}`. Mirrors what `sandhi_server_run_opts` does internally for the sync path. Daimon-side rather than calling the underscore-prefixed `_sandhi_conn_set_timeout_ms` because that's a private sandhi internal.

### Changed

- **`serve` (sync)** — replaces `sandhi_server_run(addr, port, &handle_request, 0)` with `sandhi_server_run_opts(addr, port, &handle_request, 0, opts)` where `opts = sandhi_server_options_new()` + `sandhi_server_options_idle_ms(opts, SERVE_IDLE_MS)`. sandhi's `sandhi_server_run_opts` accept loop applies `SO_RCVTIMEO = SERVE_IDLE_MS` per accepted connection (lib/sandhi.cyr:11629-11631) — same enforcement mechanism the previous default went through, just with a tighter bound.
- **`serve_async`** — calls `set_recv_timeout_ms(cfd, SERVE_IDLE_MS)` immediately after a successful `syscall(SYS_ACCEPT, sfd, 0, 0)`, before `async_spawn`. Async-handler structure unchanged.

### Security

- **VULN-async-slowloris (newly classified) — closed at 1.2.2.** Pre-1.2.2, daimon's async path had **no** per-connection timeout: `async_await_readable(cfd)` in `async_handle_client` would block indefinitely on a slow sender, and `sandhi_server_recv_request` ran without an `SO_RCVTIMEO` set on the fd. The 1.1.4 sandhi-migration audit noted the sync-path bound ("the worst case is now a 30 s `SO_RCVTIMEO` per malicious connection") but did not surface the async-path asymmetry. 1.2.2 closes this — async accepted fds now carry the same 5 000 ms `SO_RCVTIMEO` the sync path does.
- **VULN-001 / VULN-008 trade-off (carried forward from 1.1.4 audit) — improved.** The 1.1.4 ship documented the worst case as a 30 000 ms hold per malicious connection (sandhi's default `SO_RCVTIMEO`). 1.2.2 lowers that to 5 000 ms uniformly. Sandhi's `HSV_REQ_BUF_SIZE = 65 536` memory cap remains the orthogonal bound. No regression in legitimate-client behaviour expected — daimon's request handler is in-memory (no network egress in the happy path), so legitimate P99 sits well under 1 s.

### Verified

- `cyrius check --with-deps`: ok.
- `cyrius build` (DCE): **624 KB** statically-linked ELF (was 623 KB at 1.2.1; +352 bytes from the helper + opts wiring).
- `cyrius test`: **213 / 213** assertions pass (no test additions — both wirings exercise sandhi/syscall paths that aren't reachable from unit tests; integration verification is via running the binary against curl with slow-sender simulation in 1.2.x doc cleanup).
- `cyrius lint`: 0 warnings across src/ + tests/.
- `cyrius fmt`: stable.
- aarch64 cross-build: still blocked on the upstream `SYS_EPOLL_WAIT` gap (tracked upstream at [cyrius/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md](https://github.com/MacCracken/cyrius/blob/main/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md), severity **P2**); CI warn-on-detect path triggers cleanly.

### Known issues

- **`serve_async` collapse into `sandhi_server_run_opts` — still deferred upstream.** Bundled sandhi 1.3.3 (at cyrius 5.10.34) accepts `sandhi_server_options_max_conns(opts, n)` but does not honor it — the accept loop in `sandhi_server_run_opts` remains single-flight regardless of the configured value. Full write-up + auto-resolve mechanism tracked upstream at [sandhi/docs/issues/2026-05-10-daimon-server-max-conns.md](https://github.com/MacCracken/sandhi/blob/main/docs/issues/2026-05-10-daimon-server-max-conns.md) (severity **Low**). Re-checked at every cyrius pin bump; collapses to a small follow-up patch when upstream wires the enforcement (worker pool or epoll-cooperative).

## [1.2.1] - 2026-05-10

**External MCP forwarding lights up.** Replaces the `api_mcp_call` `"tool dispatch not available in sync mode"` stub at `src/main.cyr:3393` with a real `sandhi_rpc_mcp_call` dispatch path. Carryover from the 1.1.5 roadmap, sequenced as the first behavioural change of the 1.2.x arc.

### Added

- **`mcp_find_external_url(reg, name_cstr)`** — sibling lookup to `mcp_find_tool`. Returns the callback URL registered for an external tool, or `0` for builtin / not-found. Lets the dispatch site decide builtin vs external routing without re-parsing the wrapper.
- **HTTP 502 ("Bad Gateway")** added to `_http_reason` — emitted by `api_mcp_call` on upstream transport failure.

### Changed

- **`api_mcp_register`** — now requires a `callback_url` field and validates it via `validate_callback_url` (SSRF guard: http:// or https:// only). Missing/empty/non-http(s) URLs return 400 with a specific message. Pre-1.2.1 silently registered tools with empty URLs (which would then be impossible to dispatch); rejecting at the boundary surfaces the error at registration time.
- **`api_mcp_call`** — full implementation, replaces the sync-mode stub:
    - Tool not found → 400 (unchanged).
    - Tool found but builtin (no callback URL) → **501 Not Implemented** with `"builtin tool dispatch not implemented"`. Distinct from "not found" — the route exists but daimon has no in-process dispatcher for builtin tools today.
    - Tool found + external → forward to `sandhi_rpc_mcp_call(url, "tools/call", body)`. The inbound `/v1/mcp/call` body shape (`{name, arguments}`) is already the MCP `tools/call` params shape, so the body is passed through verbatim as JSON-RPC params. Sandhi wraps with `{"jsonrpc":"2.0","id":N,"method":"tools/call","params":…}`.
    - Transport failure (connect / TLS / timeout / non-2xx HTTP) → **502 Bad Gateway** with the sandhi error message embedded as JSON-escaped `upstream`.
    - JSON-RPC error envelope (`error.code != 0`) → **200 OK** with the MCP `{"content":[{"type":"text/plain","text":"…"}],"isError":true,"code":N}` shape, so MCP clients see a normal MCP error rather than a transport error.
    - Success → **200 OK** with the upstream `result` value passed through. Empty result (neither error nor result envelope) returns `{"content":[],"isError":false}`.

### Security

- **VULN-mcp-register-url (new) — closed.** `api_mcp_register` previously accepted any string (or no string at all) as `callback_url`. With `sandhi_rpc_mcp_call` now wired, an attacker who could register a tool could have caused daimon to fetch arbitrary URL schemes (`file://`, `gopher://`, internal-network HTTP — classic SSRF surface). The `validate_callback_url` allow-list (http:// + https:// only) is now enforced at the boundary. The validator was always present in `src/main.cyr:1323`; 1.2.1 wires it in.
- **No new HTTP attack surface from the dispatch path** — daimon never echoes the inbound body to the upstream; it parses the JSON for routing and hands the original cstr to sandhi as params, which sandhi wraps in a JSON-RPC envelope. JSON injection on the upstream side requires already-tampered tool registration, which now goes through the URL validator.

### Tests

- **+13 assertions** (200 → **213**). New coverage in `tests/daimon.tcyr` mcp_registry group:
    - External tool registration + name lookup + URL roundtrip.
    - `mcp_find_external_url` returns 0 for builtin and missing tools.
    - `validate_callback_url` boundary cases: null, empty, `file://`, `ftp://`, `javascript:` rejected; `http://` + `https://` accepted.

### Verified

- `cyrius check --with-deps`: ok.
- `cyrius build` (DCE): **623 KB** statically-linked ELF (1.2.0 was 622 KB; +1 360 bytes from the new dispatch path + 502 reason phrase).
- `cyrius test`: **213 / 213** assertions pass.
- `cyrius bench`: 16 microbenchmarks within noise of 1.2.0 (no benchmark touches the HTTP / MCP path).
- `cyrius lint`: 0 warnings across src/ + tests/.
- `cyrius fmt`: stable.

### Deferred (still in v1.2.x)

- **End-to-end roundtrip test** against a fake MCP server — requires a localhost listener fixture that's out of scope for tcyr unit tests. Will land alongside the v1.2.2 sandhi tuning work or as a v1.2.x rolling addition once the fixture pattern is settled.
- **Builtin tool dispatcher** — `api_mcp_call` returns 501 for builtin tools today. No in-tree consumer registers builtins through the HTTP path; if one shows up, builtin dispatch lands as its own slot.

## [1.2.0] - 2026-05-10

**Toolchain modernization + CI/release rewrite.** Bumps Cyrius 5.7.12 → 5.10.34 and sakshi 2.0.0 → 2.2.3, gitignores `/lib/`, and rebuilds CI/release on the libro/bote/agnosys 5.10.x shape. No public-API changes; same 24 endpoints, same wire shape. Internal tightening from 15 patch slots of Cyrius improvements between 5.7.12 and 5.10.34.

### Changed

- **Cyrius pin**: `5.7.12` → `5.10.34`. Covers 15 patch slots of upstream improvements (struct-by-value ABI completion, `lib/tls.cyr` early-data accessors, sandhi 1.3.3 fold-in, hashmap key-type variants, `cyrius distlib` profile bundles, `#derive(accessors)` + `#derive(Serialize)` on structs). Daimon's source compiles unchanged under the new pin — the 4 long-line lint warnings + 1 false-positive `unclosed braces` warning from 5.7.12 are all gone (no source change required).
- **Sakshi pin**: `2.0.0` → `2.2.3`. Picks up arch-portable syscalls (x86_64 + aarch64 dispatched at compile time via `_SK_SYS_*`), `sakshi_clock_recalibrate()` for long-running processes, and the 5.8.65 stdlib fold-in patch. No daimon source change — the `msg_len`-required call surface from 2.0.0 is unchanged.
- **`cyrius.cyml` stdlib deps**: added `tls`, `mmap`, `dynlib`, `fdlopen`. Required at compile time because sandhi 1.3.3's bundle unconditionally references `TLS_EARLY_DATA_ACCEPTED` for its TLS 1.3 0-RTT client-write path (`sandhi_rpc_*`). Daimon doesn't use sandhi's HTTP client today — server only — so the runtime cost is zero (DCE drops the unused paths); the deps just have to be on disk for the bundle to compile.
- **`/lib/` is now gitignored**. Repopulated by `cyrius deps` from the version-pinned stdlib snapshot + the `[deps.sakshi]` git pin. Matches the libro / bote / agnosys / patra / yukti convention. Removes 35 vendored stdlib files from the repo (binary `git diff` size shrinks substantially).
- **`src/main.cyr`** — formatter-applied: 9 continuation-line indentation fixes (4 → 8 spaces) on lines 1034-1038, 2825-2827, 3057. Cosmetic; no behavioural change. `cyrius fmt` is stable on 5.10.34 (the 5.7.12 truncation-at-line-4168 bug is fixed upstream).
- **`tests/daimon.tcyr`** — 3 long-line lint warnings cleared: dedup `else` branch split into `elif`; rag-pipeline ingest string extracted to a local; merge dup-result comment moved above the `vec_push` instead of trailing it.

### CI / Release

CI and release workflows rewritten on the **libro / bote / agnosys 5.10.x shape**:

- **Toolchain installer** — versioned layout (`~/.cyrius/versions/<V>/{bin,lib}/` + symlinks + `~/.cyrius/current`). Required by cc5 5.10.9+ which resolves arch-peer includes (`syscalls_x86_64_linux.cyr`, etc.) through this path. Pulls both the release tarball (binaries + first-party deps cache) **and** the GitHub source archive at the version tag (for the `lib/` stdlib snapshot — 5.10.x release tarballs ship `bin/` + `deps/` only).
- **`cc5_aarch64` top-level pickup** — moved out of `bin/` to the tarball top level at Cyrius 5.7.48; explicit copy step picks it up so the aarch64 cross-build keeps working.
- **Workflow env** — `CYRIUS_NO_WARN_SHADOW_LIB=1` silences the post-5.10.x `./lib/ shadows version-pinned ...` informational note across all steps; `CYRIUS_DCE=1` set at workflow level (was per-step).
- **`cc5 --version` verify** — added between install and dep-resolve.
- **`cyrius fmt` re-enabled** — `diff -q <(cyrius fmt $f) $f` per file (5.9.x+ `--check` is a no-op). Covers src/ + tests/ + bench/ + fuzz/. Daimon is clean on first run; gate fires on drift.
- **Lint flipped fail-on-warn** — `continue-on-error: true` removed. Daimon is clean under 5.10.34 (was 6 standing warnings on 5.7.12, all resolved upstream or by the cosmetic edits above).
- **Docs job** — adds `docs/doc-health.md` to the required-files list.
- **Release workflow** — mirrors agnosys release.yml: split into `ci` → `build` → `release` jobs; ships `daimon-<tag>-src.tar.gz` + `daimon-<tag>-x86_64-linux` + `daimon-<tag>-aarch64-linux` (when `cc5_aarch64` is present **and** the cross-build clears the upstream stdlib aarch64 gap — see Known issues below) + `cyrius.lock` + `SHA256SUMS`; pre-release detection on both `0.x` and `v0.x` tag styles.
- **aarch64 cross-build is tolerant of upstream stdlib gaps** — known-blocker symbols (currently `SYS_EPOLL_WAIT`) downgrade to a `::warning::` and exit 0. Any other failure still fails the step. Same posture as sakshi 2.2.2's aarch64 lane. Tracked upstream at [cyrius/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md](https://github.com/MacCracken/cyrius/blob/main/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md) (severity **P2**).

### Known issues

- **aarch64 cross-build blocked on upstream stdlib gap.** `lib/async.cyr` references `SYS_EPOLL_WAIT` unconditionally, but `lib/syscalls_aarch64_linux.cyr` only defines `SYS_EPOLL_PWAIT` (aarch64 has no plain `epoll_wait` syscall). Reproduces on both cyrius 5.10.34 and 5.10.47. Daimon's source is portable — the gap is in the cyrius stdlib. CI / release downgrade this specific error to a warning so the x86_64 ship is unblocked; aarch64 binaries return automatically when upstream patches `lib/async.cyr` or adds an arch-dispatch shim. Full write-up + workaround mechanism tracked upstream at [cyrius/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md](https://github.com/MacCracken/cyrius/blob/main/docs/development/issues/2026-05-10-daimon-async-aarch64-sys-epoll-wait.md) (severity **P2**).

### Added

- **`docs/doc-health.md`** — living ledger of doc currency (fresh / stale / read-through / evergreen / archive / open-question buckets per tier). Pattern lifted from agnosys / cyrius. Refresh discipline documented at the foot.

### Verified

- `cyrius check --with-deps src/main.cyr`: ok (one expected `shadow lib/` note silenced by `CYRIUS_NO_WARN_SHADOW_LIB=1`).
- `cyrius build src/main.cyr build/daimon` (DCE on): **622 KB** statically-linked ELF. Size delta vs 1.1.4 (452 KB) is the tls/mmap/dynlib/fdlopen modules dragged in by sandhi 1.3.3's unconditional 0-RTT constant refs — DCE drops most of the body, the headers + symbol tables account for the +170 KB.
- `cyrius test tests/daimon.tcyr`: **200 / 200** assertions pass across 26 test groups. No test changes required for the toolchain bump.
- `cyrius lint`: 0 warnings across `src/`, `tests/`, `bench/`. Was 6 warnings on 5.7.12.
- `cyrius fmt`: stable across `src/`, `tests/`, `bench/` (`diff` returns no drift).

### Roadmap

- 1.1.5 sandhi follow-ups rescoped to 1.2.1 / 1.2.2 — see `docs/development/roadmap.md`.
- "Future (v1.2.0+)" items (jnana / gRPC / WebSocket / distributed tracing / agent migration) renamed to "Future (v1.3.0+)".

## [1.1.4] - 2026-04-27

### Changed

- **HTTP server migrated to `lib/sandhi.cyr`** (Cyrius 5.7.12 stdlib). The pre-1.1.4 hand-rolled HTTP layer in `src/main.cyr` (≈580 LOC of `http_*` parse + send fns + the body-recv loop in `handle_request`) is replaced. Endpoint handlers (`api_*`) are unchanged — daimon-named shims (`http_send_response`, `http_parse_method`, `http_parse_path`, `http_parse_content_length`, `http_has_transfer_encoding`, `http_parse_query_param`, `http_parse_body`) preserve the call surface and delegate to `sandhi_server_*`.
- **Sync server (`serve`)** now delegates to `sandhi_server_run(INADDR_ANY(), port, &handle_request, 0)` — sandhi owns bind / listen / accept / recv / smuggling rejection (CL+TE conflict per RFC 7230 §3.3.3, Host.Host / CL.CL / TE.TE duplicates per §3.3.2 + §5.4) and closes the connection after the handler returns.
- **Async server (`serve_async`)** keeps its epoll-cooperative accept loop but uses `sandhi_server_recv_request`, `sandhi_server_request_has_cl_te_conflict`, and `sandhi_server_request_has_dup_smuggling_header` inline before dispatching to the same shared `handle_request`. Fresh per-call buffer (sandhi's process-global `_hsv_req_buf` is safe under the sync single-threaded loop but the async path explicitly allocates per call to keep the no-interleave invariant explicit).
- **`handle_request` is now sandhi-shape**: `(ctx, cfd, buf, blen)`. Sync caller is `sandhi_server_run`; async caller is `async_handle_client`. Same code runs under both modes.
- **CLI banner fixed** (pre-existing bug surfaced by the migration's smoke test): six `sakshi_info` / `sakshi_warn` / `sakshi_error` call sites in `src/main.cyr` were missing the required `msg_len` argument since the sakshi 2.0.0 stdlib bump. The startup banner emitted random buffer contents under sakshi as a result. Fixed by passing explicit byte lengths; the banner now reads `daimon vX.Y.Z listening on port N (mode)` cleanly.
- **`lib/http.cyr` dep dropped** from `cyrius.cyml` `[deps] stdlib`. Daimon never used the HTTP client; sandhi covers any future need.
- **CLAUDE.md** sandhi note updated from "recommended for new HTTP server work" to "in use".

### Security

Re-audited all 10 VULN findings from `docs/audit/2026-04-13-security-audit.md` against the new code path. Full report at `docs/audit/2026-04-27-sandhi-migration.md`.

- **VULN-001 (request smuggling): strengthened.** Two layers cover this end-to-end: sandhi's accept-loop rejection (CL+TE conflict, duplicate Host / CL / TE per RFC 7230) before the handler, plus daimon's continued `Transfer-Encoding` rejection inside the handler. Sandhi's `sandhi_server_content_length` is RFC 7230 §3.3.2 strict (rejects any non-digit in the value), closing the loose-digit CL.CL sub-vector that the old `http_parse_content_length` accepted (`"10, 20"` parsed as 10).
- **VULN-008 (oversized request DoS): bounded with a different latency profile.** Sandhi caps the buffer at `HSV_REQ_BUF_SIZE = 65 536` regardless of declared `Content-Length` — memory bound is preserved. The pre-1.1.4 fast-413 (parse CL on first recv, reject before reading body) is no longer reachable; the worst case is now a 30 s `SO_RCVTIMEO` per malicious connection. Net trade-off is favourable: the old path had **no** SO_RCVTIMEO, so a slowloris attacker could tie up a worker indefinitely under 1.1.3.
- **VULN-009 (per-IP rate limiting): unchanged.** `rate_check(cfd)` still runs at the top of `handle_request`, before any sandhi-side parsing.
- **VULN-002, VULN-004, VULN-005, VULN-006, VULN-010: unchanged** — none touch the HTTP path.

### Observed

- Binary size **263 KB → 452 KB** (DCE on). The reachable subset of sandhi (server fns, request parsing, header checks, smuggling rejection) is what shipped; HTTP/2, SSE, TLS, and JSON-RPC modules ride along but don't bloat (no dead-fn warnings against them in `cyrius build` output).
- 200 / 200 unit tests pass. End-to-end smoke (`/v1/health`, `/v1/agents` GET + POST, `/v1/missing`, `/v1/edge/nodes?status=online`, `/v1/metrics`) passes against both `serve` and `serve --async`. Smuggling tests via raw socket: dup-Host → 400, CL.CL with full body → 400, TE-only → 501.
- Benchmarks (16 internal microbenchmarks) within noise of 1.1.3 — none of them exercise the HTTP server, so the migration shouldn't move them and didn't.

### Deferred (tracked in `docs/development/roadmap.md` § v1.1.5)

- **External MCP forwarding** via `sandhi_rpc_mcp_call` — replaces the `"tool dispatch not available in sync mode"` stub. Needs `McpToolDescription.endpoint_url`.
- **Lower sandhi `idle_ms`** below the 30 s default once a 1.1.4 production soak surfaces a baseline P99.
- **`serve_async` collapse to `sandhi_server_run_opts`** once a Cyrius stdlib patch wires up `sandhi_server_options_max_conns` enforcement. The hook is already public — sandhi 1.0.0 (folded into Cyrius 5.7.0 stdlib, sandhi repo now in maintenance mode) deliberately landed HTTP/2 + client connection pool at 0.8.0 in favour of nailing single-server hardening first; the 0.9.x P0/P1 sweep validated that call. Wire-up is straightforward when scheduled.

## [1.1.3] - 2026-04-27

### Added

- **`[release]` table** in `cyrius.cyml` — declares `bins = ["daimon"]` + `cross_bins = ["daimon-aarch64"]` as the canonical distribution list. `release.yml` continues to drive GitHub releases directly, but future tooling (`cyrius package`, ark) reads from this single source instead of duplicating the binary list.

### Changed

- **Lint warnings reduced 6 → 2** in `src/main.cyr`. Fixed long lines:
    - `rag_config_default` prompt template extracted to a `prompt_tmpl` local (was a 119-char `str_from(...)` inline at L1075).
    - `agent_ipc_accept_one` big-endian u32 length parse wrapped across 4 lines (was a single 119-char chained `load8 * 16777216 + ...` at L2823).
    - `api_mcp_call` inline JSON literal split into 3 `str_builder_add_cstr` calls (was a single 122-char literal at L3393).
    - `ip_to_cstr` dotted-decimal print block reformatted with one statement per line (the `if (val >= 100) {...}` and `elif (val >= 10) {...}` were single 120/123-char lines at L3918-3919).
- Remaining 2 warnings are confirmed cyrius 5.7.12 linter false positives — see "Known issues" below. CI lint step stays `continue-on-error: true` until upstream fixes land.

### Known issues

- **`cyrius lint` false positive: `unclosed braces at end of file`**. The linter doesn't track string-literal state, so daimon's str-builder JSON pattern (`"{\"id\":\""` openings paired with `"}"` closings across separate calls) registers as 30 `{` vs 37 `}` overall and triggers the EOF brace warning. File parses cleanly (`cyrius check --with-deps` ok; 200/200 tests pass).
- **`cyrius lint` false positive: `trailing whitespace` at line 4177**. Comment line `# Parse remaining args: port number or --async` has zero trailing whitespace under `cat -A`, `git diff --check`, and `owl -A`. Persists after editing the comment text. Most likely a linter line-counter bug after multi-line replacements earlier in the file. Reproduces on cyrius 5.7.12.
- **`cyrius fmt` truncates `src/main.cyr`** mid-string at line 4168 on cyrius 5.7.12. Discovered when piping `cyrius fmt | cyim --write src/main.cyr` produced a 4134-line truncated file (was 4210). Hence `cyrius fmt --check` is not gated in CI; reverted via `git checkout` and unaffected since.

### Deferred (Tier 2 — separate release)

- `lib/sandhi.cyr` adoption to replace daimon's hand-rolled HTTP server (`http_parse_method`, `http_send_response`, etc., L2979-3556). Sandhi brings HTTP/2, SSE, JSON-RPC + MCP-over-HTTP (`sandhi_rpc_mcp_call`) — would also unstub daimon's external MCP forwarding in sync mode (currently returns the "tool dispatch not available in sync mode" error reformatted in this patch).
- `lib/atomic.cyr` for circuit-breaker counters in `cb_*` (L477-548): reviewed and explicitly skipped — both sync and async (epoll-cooperative) HTTP modes are single-threaded, so there is no concurrent reader/writer to atomicize. Adding atomics would solve a non-problem.
- `lib/sankoch.cyr` for memory-store compaction: deferred to its own release with a benchmark cycle.

## [1.1.2] - 2026-04-27

### Changed

- **Cyrius toolchain bump**: `4.5.0` → `5.7.12`. Build remains clean (`cyrius check --with-deps`), test suite holds (200 passed / 0 failed), binary builds (`build/daimon`, ~263 KB statically linked).
- **Manifest migration**: `cyrius.toml` → `cyrius.cyml` (5.0.0 breaking; `cyrius update` semantics). `version` now resolves from the `VERSION` file via `${file:VERSION}`. `output` moved under `build/` to match the modern layout used by vidya 2.3.0.
- **`sakshi` is no longer stdlib**: dropped from `[deps] stdlib`, added as `[deps.sakshi]` git pin (`tag = "2.0.0"`). After `cyrius deps`, `lib/sakshi.cyr` is a symlink into `~/.cyrius/deps/sakshi/2.0.0/dist/sakshi.cyr`.
- **`math` added to stdlib deps**: `f64_sqrt` (used by `vector_normalize` and `cosine_similarity`) needed `lib/math.cyr` registered explicitly so `cyrius vet` resolves it; build was implicitly pulling it in via stdlib auto-prepend, vet now reports `2 deps, 0 untrusted, 0 missing`.
- `cyrius.lock` now committed (5.7.8 made lockfile-on-by-default).
- CLAUDE.md: updated Cyrius pin reference (4.2.0 → 5.7.12), added note that `lib/sandhi.cyr` is the recommended HTTP path for new server work, documented `hashmap` key-type variants and external sakshi dep.

### CI / Release

- **CI overhauled** (`.github/workflows/ci.yml`) modeled on the modern agnostik 5.7.x shape: toolchain version pulled from `cyrius.cyml` (was `.cyrius-toolchain`); added `cyrius deps --verify`, `cyrius vet`, DCE build (`CYRIUS_DCE=1`), ELF magic check, best-effort aarch64 cross-build, per-test loop with discrete failure visibility, `cyrius bench` step, security-scan job (no `sys_system`, no writes to `/etc|/bin|/sbin`, no ≥64 KB stack buffers — comment-aware), and a docs job that checks 12 required files + version-in-CHANGELOG consistency.
- **`cyrius lint` runs as advisory** (`continue-on-error: true`) — daimon has 6 standing warnings (4 long lines + 1 false-positive brace from JSON-in-string-literal at line 3393). Tier-3 cleanup will eliminate them and flip lint to fail-on-warn.
- **`cyrius fmt --check` not gated**: cyrius 5.7.12 fmt has a truncation bug on daimon's `src/main.cyr` (cuts mid-string at line 4168). Skipping the gate until the upstream fix lands.
- **Release workflow rewritten** (`.github/workflows/release.yml`) to mirror agnostik: accepts both `v1.2.3` and `1.2.3` tag styles, semver-shape verification, source tarball + cross-arch binaries + `SHA256SUMS` archive, per-version changelog extraction via awk, pre-release detection for `0.x` tags.
- **`scripts/version-bump.sh`** simplified — only writes `VERSION` now, since `cyrius.cyml` resolves `[package].version` from it via `${file:VERSION}`.
- **README.md / CONTRIBUTING.md** updated for `cyrius.cyml` + new pin.

## [1.1.1] - 2026-04-13

### Changed

- Roadmap: unblocked nein-core firewall work. [nein](https://github.com/MacCracken/nein) v0.1.0 Cyrius port shipped 13 modules (rule/table/chain/set/nat/bridge/engine/mesh/geoip/policy/builder/firewall/validate) — daimon can now depend on nein directly. Nein's own `mcp` module stays gated on bote.

## [1.1.0] - 2026-04-13

### Added

- **Async HTTP server** — `serve --async` flag enables epoll-based cooperative concurrency via `lib/async.cyr`. Handles multiple connections per accept cycle with batched async_run. Both sync and async modes share the same request handler and security controls.
- Documentation: architecture overview, API guide with all 24 endpoints, quickstart guide, 3 ADRs (port rationale, HTTP mode, security process).
- Vendored `lib/async.cyr` (epoll cooperative runtime), `lib/http.cyr` (HTTP client), `lib/thread.cyr`, `lib/callback.cyr` via updated `cyrius.toml` deps.

### Changed

- Removed `rust-old/` directory (16 GB, 9,724 LOC Rust source + build cache). Rust history available in git pre-v0.7.0 tags.
- Refactored serve loop: request handling extracted to `handle_request(cfd)`, server setup to `server_bind(port)` — shared by both sync and async modes.
- SECURITY.md updated for v1.x supported versions, Cyrius-specific scope.
- CLI: `serve [port] [--async]` — async mode optional, sync remains default.

## [0.7.0] - 2026-04-13

Complete rewrite from Rust to Cyrius. 9,724 LOC Rust → 4,141 LOC Cyrius. Binary: 181 KB (was 4.0 MB). Zero external dependencies.

### Added

- **15 modules ported** with full API parity: error, config, agent, supervisor, memory, vector_store, rag, mcp, screen, scheduler, federation, edge, ipc, api, logging.
- **24 HTTP API endpoints** — synchronous TCP server on port 8090:
  - `/v1/health` — service health
  - `/v1/agents` (GET/POST), `/v1/agents/{id}` (GET) — agent lifecycle
  - `/v1/mcp/tools` (GET/POST), `/v1/mcp/tools/{name}` (DELETE), `/v1/mcp/call` (POST) — MCP tool dispatch
  - `/v1/rag/ingest` (POST), `/v1/rag/query` (POST) — RAG pipeline
  - `/v1/edge/nodes` (GET/POST), `/v1/edge/nodes/{id}` (GET), `…/heartbeat` (POST), `…/decommission` (POST), `/v1/edge/stats` — edge fleet
  - `/v1/scheduler/tasks` (GET/POST), `…/{id}` (GET), `…/{id}/cancel` (POST), `/v1/scheduler/nodes` (POST), `/v1/scheduler/schedule` (POST), `/v1/scheduler/stats` — task scheduling
  - `/v1/metrics` — aggregate metrics
- **Scheduler**: NodeCapacity with resource fitting + bin-packing, schedule_pending with assignment decisions, preempt_check, tasks_for_node, CronScheduler with interval-based entries + validation, stats aggregation.
- **Federation**: cluster management, heartbeat health tracking (online/suspect/dead), Raft-like election (start_election, receive_vote_request, receive_vote, become_coordinator, step_down), 4-factor weighted node scoring (resource/locality/load/affinity), agent placement, cluster stats.
- **Edge fleet**: register with validation (empty name, duplicate, fleet-full), heartbeat, health check (suspect/offline thresholds), decommission, list with status filter, stats.
- **FederatedVectorStore**: collection/replica management, cross-node search merge with dedup + re-ranking, remove_node, stats.
- **IPC**: Unix domain socket AgentIpc (bind/accept/send, length-prefixed wire protocol, ACK/NACK, connection limits), message bus (named routing + broadcast + direct send), RPC registry.
- **Memory store**: CRUD with atomic write (tmp+rename), list_keys, list_by_tag, clear, usage_bytes, key validation + sanitization.
- **RAG pipeline**: ingest_text (chunk + embed + index), query_text (embed + search + format context).
- **Vector store**: cosine similarity, brute-force search with ranking, normalize_vec.
- **Agent lifecycle**: start/stop/pause/resume with race-free pidfd signal delivery, /proc resource monitoring (VmRSS, CPU time, fd count, thread count), resource limits on spawned processes.
- **Supervisor**: circuit breaker (Closed→Open→HalfOpen), output capture ring buffer, resource quotas, health tracking.
- **MCP**: tool registry (builtin + external) with manifest, register, deregister, validate_callback_url.
- **Screen capture**: permission manager with rate limiting, recording sessions (active/paused/stopped).
- CLI: `serve [port]`, `version`, `help`.
- Test suite: 200 assertions / 26 test groups.
- Benchmark suite: 16 benchmarks with Rust comparison (BENCHMARKS.md).
- Fuzz harnesses: 5 (circuit_breaker, memory_keys, scheduler_fsm, vector_store, mcp_registry).
- Security audit: docs/audit/2026-04-13-security-audit.md — 10 findings, 9 fixed, 1 accepted risk.

### Security

- **VULN-001**: Content-Length validation, Transfer-Encoding rejection (501), 413 Payload Too Large. Prevents request smuggling.
- **VULN-002**: `json_escape_str()` on all user-controlled strings in JSON responses. Prevents JSON injection.
- **VULN-004**: `pidfd_open()`/`pidfd_send_signal()` with `kill()` fallback. Prevents PID reuse race.
- **VULN-005**: Agent memory directories 0700 (was 0755).
- **VULN-006**: `SO_PEERCRED` UID verification on Unix socket accept. Prevents unauthorized IPC.
- **VULN-008**: `MAX_REQUEST_SIZE=65536`, Content-Length body reads. Prevents oversized request DoS.
- **VULN-009**: Per-IP rate limiting — 120 req/min sliding window, 429 Too Many Requests.
- **VULN-010**: `agent_spawn_with_limits()` with `RLIMIT_AS` + `RLIMIT_CPU`. Prevents agent resource exhaustion.
- HTTP query parameter bounds checking. Prevents buffer over-read.
- Empty path segment handling (`/v1/agents/` → 404).

### Changed

- **Language**: Rust → Cyrius. Rust source removed in v1.0.1.
- **Toolchain**: Cyrius 4.2.0 (pinned in `.cyrius-toolchain`).
- **Build**: `cargo build` → `cyrius build src/main.cyr build/daimon`.
- **HTTP**: Async (tokio/axum) → synchronous (raw TCP sockets).
- **Dependencies**: 193 crate dependencies → 17 Cyrius stdlib modules + 0 external.
- **Binary**: 4.0 MB → 181 KB (96% smaller).

### Breaking

- Language changed from Rust to Cyrius. Consumers must use Cyrius 4.2.0+ to build.
- HTTP server is synchronous (single-threaded). No concurrent request handling.
- MCP tool call forwarding returns error stub — blocked on bote Cyrius port.
- Firewall MCP tools not available — blocked on nein Cyrius port.

## [0.6.0] - 2026-04-03

### Added

- `http-forward` feature gate — external MCP tool forwarding via reqwest is now opt-in.
- `Serialize`/`Deserialize` on 8 previously non-serializable types.
- Input validation: positive CPU/memory on scheduler node registration, cron bounds.
- 13 new tests (305 total), 4 new benchmarks (19 total).

### Fixed

- **Security**: External MCP tool-not-found → 400 (was leaking 404).
- **Security**: Firewall table filter exact match (was substring).
- **Safety**: `setrlimit` return values checked, scheduler `.get()` instead of index, RpcRouter mutex handling.

### Changed

- Binary size: 12 MB → 4.0 MB (−64%) default, 8.2 MB (−32%) with http-forward.
- Dependencies: 354 → 193 (−45%). Dropped anyhow, async-trait.
- `Supervisor::check_health` now generic.

## [0.5.0] - 2026-03-26

### Added

- Full axum HTTP API router with 20+ endpoints.
- Integration test suite (28 tests), benchmark suite (15 benchmarks).

### Fixed

- Restrictive CORS, timestamp correctness, cron time matching, socket permission logging.

## [0.1.0] - 2026-03-25

### Added

- Initial scaffold — all modules extracted from agnosticos monorepo.
