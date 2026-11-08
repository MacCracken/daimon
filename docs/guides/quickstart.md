# Quick Start

## Prerequisites

- [Cyrius](https://github.com/MacCracken/cyrius) 6.7.18, the pin in `cyrius.cyml`
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

## Keep State, and Index Files

What outlives a restart goes under `--data-dir` (default `/var/lib/agnos`). Without one daimon can
write, it still runs, and says what will not last: the manifest's `pins.state` reads `unavailable`.

```bash
# Index a directory. Off by default: anything local can read what the index holds (see the API guide).
./build/daimon serve --data-dir ~/.local/share/daimon --index-root "$HOME/Documents"

curl -s 127.0.0.1:8090/v1/files/index            # none, building, catching_up, paused or fresh
curl -s '127.0.0.1:8090/v1/files?untagged=1&limit=20'
curl -s -X POST 127.0.0.1:8090/v1/files/tags -d "{\"path\":\"$HOME/Documents/a.txt\",\"add\":[\"wip\"]}"
curl -s 127.0.0.1:8090/v1/files/smart            # saved queries, each with a count
curl -s '127.0.0.1:8090/v1/files?q=quarterly+report'   # a question, ranked. Names only, unless:
```

`serve --index-content` also searches what files say. That is a second opt-in: anything local can then
ask whether a file holds a word.

Search can rank by meaning, so `receipt` finds a file about an invoice that never says the word. It needs a table of word
vectors, which you build once from any GloVe, word2vec or fastText text file:

```bash
./build/daimon vectors build glove.6B.100d.txt ~/.local/share/daimon/vectors --name glove-6B-100d
./build/daimon serve --data-dir ~/.local/share/daimon --index-root "$HOME/Documents" --index-content \
    --index-vectors ~/.local/share/daimon/vectors
```

## Run Tests

```bash
sh tests/test.sh                  # every suite, the HTTP smoke and the fuzz harnesses
cyrius test tests                 # every tests/*.tcyr suite. Judge a suite by its exit status: it prints its
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
src/                  Source: 34 modules, entry src/main.cyr
tests/*.tcyr          22 test suites, 2172 checks, each including the real src/ module it tests
tests/smoke.sh        HTTP smoke checks against the built binary (300); runs tests/containment.sh
tests/agnos/          The AGNOS guest test: sh tests/agnos/run.sh --release boots agnos under QEMU
tests/aarch64/        The aarch64 VM test: every suite and a smoke of the binary under a real kernel
tests/daimon.bcyr     Benchmarks (37; tests/rag_ingest.bcyr has 2 more)
fuzz/                 Property-based fuzz harnesses (10), sharing fuzz/rng.cyr
build/daimon          The binary: 2,636,472 bytes, static; unreachable code removed (2.5.1)
docs/                 Architecture, guides, ADRs, audit reports, roadmap
```
