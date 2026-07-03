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
cyrius build tests/daimon.tcyr build/daimon_test && build/daimon_test
# → 225 passed, 0 failed (225 total)
```

## Run Benchmarks

```bash
cyrius build tests/daimon.bcyr build/daimon_bench && build/daimon_bench
```

## Project Structure

```
src/                  Source (28 modules, entry src/main.cyr)
tests/daimon.tcyr     Test suite (225 assertions)
tests/daimon.bcyr     Benchmarks (17)
fuzz/                 Fuzz harnesses (5)
build/daimon          Binary (181 KB)
docs/                 Architecture, guides, ADRs, audit reports
```
