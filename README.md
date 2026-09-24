# Daimon

**Daimon** (Greek: δαίμων — guiding spirit) — the AGNOS agent orchestrator.

daimon starts, supervises and stops AGNOS agents, and serves them and their clients one HTTP API
(`127.0.0.1:8090`). It is written in [Cyrius](https://github.com/MacCracken/cyrius), ported from
Rust. AGNOS is its primary target; it also runs on Linux, x86_64 and aarch64.

## What it does

- **Agent lifecycle.** Register, start, stop, pause, resume and delete agents over the API. daimon
  runs the executable installed for the agent's type, never one a request names, under rlimits and
  in a process group of its own. Where daimon's cgroup is its own to divide, each agent also gets a
  cgroup, so a stop reaches everything the agent started
  ([ADR-004](docs/adr/004-agent-process-control.md), [ADR-008](docs/adr/008-agent-containment-cgroup.md)).
- **Agent channels.** Every agent daimon starts has a channel to it, open as its fd 3. Agents write
  length-prefixed JSON frames there, which daimon routes onto a message bus that HTTP clients read
  and write ([agent-ipc.md](docs/guides/agent-ipc.md)). With `serve --agent-output capture`, daimon
  also keeps each agent's stdout and stderr.
- **MCP host.** daimon hosts 13 built-in tools: libro's five audit-chain tools, `web_fetch` and
  `web_search`, and nein's six firewall tools. The two tools that change the firewall are refused
  until callers are authenticated (roadmap 2.5.x). External MCP servers register tools, resources
  and prompts, and daimon forwards calls to them.
- **Audit trail.** daimon records its lifecycle and security events on a hash-linked
  [libro](https://github.com/MacCracken/libro) chain, which the `libro_*` tools query, verify and
  export.
- **Task scheduling.** [samay](https://github.com/MacCracken/samay) places tasks on compute nodes by
  best fit, and each node's executor starts and completes them.
- **RAG**: ingest text, then query it, over an in-memory vector index.
- **Edge fleet**: node registration, heartbeats, health and fleet statistics.
- **Tracing.** With `serve --trace`, daimon adopts and propagates W3C `traceparent` and emits
  [sakshi](https://github.com/MacCracken/sakshi) spans.

The federation, screen-capture and per-agent memory modules came with the port and have their own
tests, but no route reaches them yet. The memory store waits for agent identity (roadmap 2.5.x).

daimon runs as one thread with its own event loop ([ADR-006](docs/adr/006-own-event-loop.md)). A
slow client holds only its own connection, and a call that waits on another server runs in a child
process.

**On AGNOS** (2.4.0) daimon starts, stops and hears its agents with the kernel's own primitives,
and (2.4.1) runs its calls to other servers in a child, as on Linux. Some things agnos does not
provide yet, such as resource limits, a way to end a process, or a loopback-only listener. Each gap
is filed with agnos. What daimon does meanwhile is in [ADR-007](docs/adr/007-daimon-on-agnos.md) and
the [AGNOS audit](docs/audit/2026-09-23-agnos-platform-audit.md).

## Security

The API has **no authentication** until roadmap 2.5.x. To limit who can reach it, it:
- binds 127.0.0.1 unless `serve --listen` says otherwise;
- while it listens on loopback, answers only requests that name a loopback host;
- refuses a state change sent from another site's page;
- refuses agent and task control from any browser.

See [SECURITY.md](SECURITY.md) and the audit reports in [docs/audit/](docs/audit/).

## Building

Requires [Cyrius](https://github.com/MacCracken/cyrius) 6.6.6, the pin in `cyrius.cyml`.

```bash
cyrius deps                             # vendor the stdlib subset and the git deps into lib/ (gitignored)
cyrius build src/main.cyr build/daimon  # also --aarch64 build/daimon-aarch64, --agnos build/daimon-agnos
./build/daimon serve                    # 127.0.0.1:8090; a port, or --listen ADDR, to change it
./build/daimon help                     # every flag (--agents-dir, --agent-env, --agent-output, --trace, ...)
```

`cyrius deps` resolves sakshi, bote, libro, majra, sigil, bayan, samay, nein and ai-hwaccel at the
tags in `cyrius.cyml`, and every build runs it first. samay and nein also name a sibling checkout
(`path = "../samay"`, `"../nein"`); when that checkout is present, `cyrius deps` uses it instead of
the tag.

Each tagged release publishes:
- the source tarball;
- the x86_64 Linux binary, and the aarch64 one when the toolchain can cross-build it;
- `cyrius.lock` and `SHA256SUMS`.

## Testing

```bash
sh tests/test.sh                    # every suite, the HTTP smoke and the fuzz harnesses
cyrius tests                        # every suite in tests/: 1087 checks, 17 files
cyrius fuzz                         # 7 property-based harnesses, 176,349 generated cases
cyrius bench tests/daimon.bcyr      # 28 benchmarks, one only where agents can be contained (+2 in tests/rag_ingest.bcyr)
sh tests/smoke.sh                   # the linked binary over HTTP, 143 checks: libro, tracing, regressions,
                                    # the agent lifecycle, the bind address, task start/complete,
                                    # request-string decoding, agent channels and messages,
                                    # the event loop, hosts and origins, captured output,
                                    # detached MCP calls, connection slots, agent containment
sh tests/agnos/run.sh --release     # on AGNOS: boots the released agnos kernel under QEMU and runs
                                    # daimon's agent lifecycle, channels and API there (92 checks).
                                    # Without --release: ../agnos's build
sh tests/aarch64/run.sh             # every suite, then a smoke of the binary, in an aarch64 VM
```

CI runs all of these on every push. It runs the benchmarks too, but does not compare their numbers.
It also verifies the committed `cyrius.lock`.

Every suite, benchmark and fuzz harness includes the real `src/` module it tests. None carries a
copy of daimon's code: that was the 2.2.x test-integrity arc, and the CHANGELOG records what it
found.

## Documentation

- [Quickstart](docs/guides/quickstart.md) · [API reference](docs/guides/api.md) · [Agent IPC](docs/guides/agent-ipc.md) · [Architecture](docs/architecture/overview.md) · [Roadmap](docs/development/roadmap.md)
- [Decision records](docs/adr/) · [Security audits](docs/audit/) · [Benchmarks](BENCHMARKS.md)
- [CHANGELOG](CHANGELOG.md) · [Security policy](SECURITY.md) · [Contributing](CONTRIBUTING.md)

## Status

At 2.4.3, `src/` is 10,044 lines in 30 modules. The port (0.7.0) turned 9,724 lines of Rust into
4,141 of Cyrius. The binaries are statically linked:

| target | bytes |
|---|---:|
| x86_64 | 3,313,248 |
| aarch64 | 4,209,992 |
| agnos | 3,242,848 |

Of x86_64's bytes, 1,623,422 are unreachable code that the toolchain NOPs in place rather than
removing (CHANGELOG 1.2.4). The 181 KB this line once gave was the v1.0.1 port-era build (cyrius
4.2.0, BENCHMARKS.md).

Next on the [roadmap](docs/development/roadmap.md): agent identity and MCP caller authentication
(2.5.x), then per-agent arena isolation (3.0.0).

## License

GPL-3.0-only
