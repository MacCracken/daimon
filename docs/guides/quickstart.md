# Quick Start

## Prerequisites

- [Cyrius](https://github.com/MacCracken/cyrius) 6.6.6, the pin in `cyrius.cyml`
- Linux, x86_64 or aarch64, to build and run it here. For AGNOS, build with `--agnos`; see
  [Run Tests](#run-tests) for the guest test

## Build

```bash
cyrius deps                             # vendors the stdlib subset and the git deps into lib/ (gitignored)
cyrius build src/main.cyr build/daimon  # every build runs `cyrius deps` first
```

## Run

```bash
# Start on the default 127.0.0.1:8090 (--listen 0.0.0.0 accepts other hosts; there is no authentication)
./build/daimon serve

# Start on another port
./build/daimon serve 9090

# Check the version, or list every flag
./build/daimon version
./build/daimon help
```

## Verify

```bash
curl http://127.0.0.1:8090/v1/health
# → {"status":"ok","agents":0,"mcp_tools":13,"mcp_resources":0,"mcp_prompts":0,"edge_nodes":0}
```

The 13 tools are daimon's builtins. `curl http://127.0.0.1:8090/v1/mcp/tools` lists them.

## Register an Agent

```bash
curl -X POST http://127.0.0.1:8090/v1/agents -d '{"name":"my-first-agent","type":"User"}'
# → {"id":1,"name":"my-first-agent","type":"User","status":0}
```

## Start and Stop It

daimon runs the executable installed for the agent's type, here `agnos-agent-user-agent`. It looks
in `/usr/lib/agnos/agents`, then `/opt/agnos/agents`, and falls back to
`/usr/bin/agnos-agent-runner`. Start the server with `--agents-dir DIR` to use your own directory.

```bash
curl -X POST http://127.0.0.1:8090/v1/agents/1/start
# → {"id":1,"name":"my-first-agent","type":"User","status":2,"pid":4242,"exit_code":null,"limits_enforced":true,"contained":false}
curl -X POST http://127.0.0.1:8090/v1/agents/1/stop     # SIGTERM, up to 5 s, then SIGKILL
# → {"id":1,"name":"my-first-agent","type":"User","status":5,"pid":0,"exit_code":143}
```

`exit_code` is the agent's own: 0 for one that exits cleanly on SIGTERM, and 143 (128 + 15) for one
the signal killed. `"contained"` is `true` where daimon runs in a cgroup it may divide, such as a
`systemd-run --user` scope. From a login shell it is `false`, and daimon warns once at startup. See
the [API guide](api.md#agents) for pause, resume, delete, the status codes and containment.

A started agent can send daimon messages on its fd 3 (`AGNOS_IPC_FD=3`): a 4-byte big-endian length,
then a JSON object naming a `target`. Read an agent's queue with `POST /v1/agents/1/messages/take`.
See [agent-ipc.md](agent-ipc.md).

To see what agents print, start daimon with `serve --agent-output capture` and read
`GET /v1/agents/1/output`.

## Run Tests

```bash
sh tests/test.sh                  # every suite, the HTTP smoke and the fuzz harnesses
cyrius tests                      # every tests/*.tcyr suite. Judge a suite by its exit status: it prints its
                                  # "N passed, 0 failed" line before it exits
cyrius fuzz                       # the fuzz/ harnesses
sh tests/smoke.sh                 # the built binary over HTTP
sh tests/agnos/run.sh --release   # the AGNOS guest test: the released agnos kernel under QEMU
sh tests/aarch64/run.sh           # every suite and a smoke of the binary in an aarch64 VM
```

## Run Benchmarks

```bash
cyrius bench tests/daimon.bcyr
./scripts/bench-history.sh        # the same, appended to bench-history.csv (gitignored)
```

## Project Structure

```
src/                  Source: 30 modules, entry src/main.cyr
tests/*.tcyr          17 test suites, 1087 checks, each including the real src/ module it tests
tests/smoke.sh        HTTP smoke checks against the built binary (143); runs tests/containment.sh
tests/agnos/          The AGNOS guest test: sh tests/agnos/run.sh --release boots agnos under QEMU
tests/aarch64/        The aarch64 VM test: every suite and a smoke of the binary under a real kernel
tests/daimon.bcyr     Benchmarks (28; tests/rag_ingest.bcyr has 2 more)
fuzz/                 Property-based fuzz harnesses (7), sharing fuzz/rng.cyr
build/daimon          The binary: 3,313,248 bytes, static; 1,623,422 of them NOPed unreachable code
docs/                 Architecture, guides, ADRs, audit reports, roadmap
```
