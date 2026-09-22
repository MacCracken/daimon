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
# Start on default port 8090
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
curl -X POST http://localhost:8090/v1/agents -d '{"name":"my-first-agent"}'
# → {"id":1,"name":"my-first-agent","status":0}
```

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
tests/*.tcyr          Test suites, one per module (645 assertions, 16 files)
tests/daimon.bcyr     Benchmarks (19; tests/rag_ingest.bcyr has 2 more)
tests/smoke.sh        HTTP smoke checks against the built binary
fuzz/                 Property-based fuzz harnesses (6), sharing fuzz/rng.cyr
build/daimon          Binary (181 KB)
docs/                 Architecture, guides, ADRs, audit reports
```
