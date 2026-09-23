# Contributing to Daimon

Thank you for your interest in contributing to Daimon! This document outlines how
to get involved.

## Getting Started

1. Fork the repository and clone your fork
2. Install [Cyrius](https://github.com/MacCracken/cyrius) 6.6.6, the pin in `cyrius.cyml` (`[package].cyrius`)
3. Build and run `sh tests/test.sh` (below) to verify your environment

## Development Workflow

```bash
export CYRIUS_NO_WARN_SHADOW_LIB=1 CYRIUS_DCE=1   # what CI sets
cyrius lib sync                      # Vendor the stdlib subset from the pin
cyrius deps                          # Resolve git dependencies (e.g. sakshi)
cyrius build src/main.cyr build/daimon  # Build (also --agnos, --aarch64)
cyrius vet src/main.cyr              # Include dependencies
cyrius fmt <file> --check            # Formatting (without --check it rewrites the file)
cyrius lint <file>                   # CI fails on any warning
cyrius tests                         # Run every test suite
cyrius fuzz                          # Run the fuzz harnesses
cyrius bench tests/daimon.bcyr       # Run benchmarks
sh tests/test.sh                     # Tests + fuzz + HTTP smoke (tests/smoke.sh)
./scripts/bench-history.sh           # Append benchmark baseline
sh tests/agnos/run.sh                # AGNOS guest test (needs a built agnos kernel and gnoboot)
```

A change to an agnos arm (`#ifdef CYRIUS_TARGET_AGNOS`) is tested on agnos:
`tests/agnos/run.sh` boots the kernel under QEMU and runs `tests/agnos/guest.cyr` and the real
daimon, driven over HTTP by `tests/agnos/http_client.cyr`. On agnos, wait with `daimon_yield_ms`,
never `sleep_ms`, which holds the CPU (ADR-007).

`lib/` is gitignored — it is repopulated by `cyrius lib sync` (stdlib subset from
the pin) and `cyrius deps` (git deps like sakshi). Run both after cloning.

## Pull Requests

- Keep PRs focused — one feature or fix per PR
- Add tests for new logic
- Run the gate before submitting: `cyrius vet`, `cyrius fmt --check` and `cyrius lint` on every
  file you touched, the three builds, and `sh tests/test.sh`. (`cyrius check` is only a syntax
  check.)
- Update `CHANGELOG.md` under an `[Unreleased]` heading

## Code Style

- Follow existing patterns in the codebase
- Use `Result`/`Option` tagged unions for error handling — avoid crashing
- Use accessor functions for struct fields
- Keep functions focused and small
- Use unique variable names within a function (cyrius has no block scoping)

## Adding a New Module

1. Add the module code under `src/` (a new `src/*.cyr` module)
2. Add its suite as `tests/<module>.tcyr`, which **`include`s `src/<module>.cyr`** and its real
   dependencies — never a copy of the functions under test. Until 2.2.3 `tests/daimon.tcyr`
   tested simplified local copies, and two security defects shipped green because the copies did
   not contain them; moving the last modules onto their real source found more (2.2.3 CHANGELOG).
   A suite that includes `src/agent.cyr` must also include `src/ipc.cyr` (agent starts open
   channels). End a suite with `syscall(SYS_EXIT_GROUP, assert_summary())`, the epilogue
   `lib/syscalls.cyr` prescribes: with `SYS_EXIT` only the calling thread ends, and a suite that
   ever started one never finishes (2.3.3 learned that). A fixture agent's own children must not
   keep the suite's output (redirect them): a regression that leaves one running would otherwise
   make the harness wait for it.
3. Add benchmarks in `tests/daimon.bcyr` if performance-relevant — against the real functions too
4. Add a fuzz harness in `fuzz/` for code that takes untrusted input: drive the real module with
   `fuzz/rng.cyr`, check properties from the documented contract, and exit with the number of the
   property that broke (`sys_exit(n)` — never a raw `syscall(60, …)`, which is x86-only)

## Adding an HTTP Endpoint

1. Route it in `src/router.cyr` with an explicit method check (`route_method`). A state change never
   rides a GET.
2. Read the body with `http_body_json` and the `http_json_str` / `_int` / `_has` / `_text` readers
   (`src/http.cyr`), and answer `http_bad_body` when `http_body_json` refuses it. Do **not** use
   bayan's flat `json_parse`. It keeps JSON escapes undecoded and misreads nested objects, which is
   why daimon stopped using it at 2.3.2 (CHANGELOG 2.3.2).
3. Escape every string you echo with `json_escape_str` (`json_escape_text` for bytes that are not
   known to be UTF-8, such as an agent's output).
4. The Host allowlist and the cross-site-write refusal already cover every route
   (`handle_request`); route agent or task control through `_route_refuse_browser` as well.
5. Do not block. A handler runs on daimon's one event loop, so waiting for a process or a peer holds
   every other client and every agent channel. Answer later instead (`server_defer`, as
   `api_agent_stop` does).
6. Add checks to `tests/smoke.sh`, which drives the linked binary over HTTP.

## Reporting Issues

Open an issue on [GitHub](https://github.com/MacCracken/daimon/issues) with:
- What you expected to happen
- What actually happened
- Steps to reproduce
- Cyrius version (`cyrius --version`)

## License

By contributing, you agree that your contributions will be licensed under
GPL-3.0-only, consistent with the project license.
