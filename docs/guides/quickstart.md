# Quick Start

## Prerequisites

- [Cyrius](https://github.com/MacCracken/cyrius) 6.3.43+
- Linux x86_64 or aarch64

## Build

```bash
# lib/ is gitignored — repopulate it before building:
cyrius lib sync   # vendors the stdlib subset from the cyrius pin
cyrius deps       # resolves git deps (e.g. sakshi) into lib/
cyrius build src/main.cyr build/daimon
```

## Run

```bash
# Start on the default 127.0.0.1:8090 (--listen 0.0.0.0 to accept other hosts)
./build/daimon serve

# Start on custom port
./build/daimon serve 9090

# Check version
./build/daimon version
```

## Verify

```bash
curl http://localhost:8090/v1/health
# → {"status":"ok","agents":0,"mcp_tools":0,"edge_nodes":0}
```

## Register an Agent

```bash
curl -X POST http://localhost:8090/v1/agents -d '{"name":"my-first-agent","type":"User"}'
# → {"id":1,"name":"my-first-agent","type":"User","status":0}
```

## Start and Stop It

daimon runs the executable installed for the agent's type, `agnos-agent-user-agent` here. It looks
in `/usr/lib/agnos/agents`, then `/opt/agnos/agents`, and falls back to
`/usr/bin/agnos-agent-runner`. Start the server with `--agents-dir DIR` to use your own directory.

```bash
curl -X POST http://localhost:8090/v1/agents/1/start
# → {"id":1,"name":"my-first-agent","type":"User","status":2,"pid":4242,"exit_code":null}
curl -X POST http://localhost:8090/v1/agents/1/stop     # SIGTERM, up to 5 s, then SIGKILL
# → {"id":1,"name":"my-first-agent","type":"User","status":5,"pid":0,"exit_code":0}
```

See the [API guide](api.md#agents) for pause, resume, delete and the status codes.

## Run Tests

```bash
cyrius tests          # every tests/*.tcyr suite — each prints "N passed, 0 failed"
cyrius fuzz           # the fuzz/ harnesses
sh tests/smoke.sh     # the built binary over HTTP
```

## Run Benchmarks

```bash
cyrius bench tests/daimon.bcyr
```

## Project Structure

```
src/                  Source (30 modules, entry src/main.cyr)
tests/*.tcyr          Test suites, one per module (833 assertions, 17 files)
tests/daimon.bcyr     Benchmarks (24; tests/rag_ingest.bcyr has 2 more)
tests/smoke.sh        HTTP smoke checks against the built binary
fuzz/                 Property-based fuzz harnesses (7), sharing fuzz/rng.cyr
build/daimon          Binary (3.2 MB, static; ~1.6 MB of it NOPed unreachable code)
docs/                 Architecture, guides, ADRs, audit reports
```
