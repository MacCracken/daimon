# Contributing to Daimon

Thank you for your interest in contributing to Daimon! This document outlines how
to get involved.

## Getting Started

1. Fork the repository and clone your fork
2. Install [Cyrius](https://github.com/MacCracken/cyrius) 5.7.12+ (see `cyrius.cyml` `[package].cyrius`)
3. Run `cyrius check` to verify your environment

## Development Workflow

```bash
cyrius deps                          # Resolve dependencies
cyrius build                         # Build (reads cyrius.cyml)
cyrius check                         # Format + lint + test + build
cyrius test tests/daimon.tcyr        # Run test suite
cyrius bench tests/daimon.bcyr       # Run benchmarks
sh tests/test.sh                     # Tests + fuzz harnesses
./scripts/bench-history.sh           # Append benchmark baseline
```

## Pull Requests

- Keep PRs focused — one feature or fix per PR
- Add tests for new logic
- Run `cyrius check` before submitting
- Update `CHANGELOG.md` under an `[Unreleased]` heading

## Code Style

- Follow existing patterns in the codebase
- Use `Result`/`Option` tagged unions for error handling — avoid crashing
- Use accessor functions for struct fields
- Keep functions focused and small
- Use unique variable names within a function (cyrius has no block scoping)

## Adding a New Module

1. Add the module code to `src/main.cyr`
2. Add tests in `tests/daimon.tcyr`
3. Add benchmarks in `tests/daimon.bcyr` if performance-relevant
4. Add fuzz harnesses in `fuzz/` for security-critical code

## Reporting Issues

Open an issue on [GitHub](https://github.com/MacCracken/daimon/issues) with:
- What you expected to happen
- What actually happened
- Steps to reproduce
- Cyrius version (`cyrius --version`)

## License

By contributing, you agree that your contributions will be licensed under
GPL-3.0-only, consistent with the project license.
