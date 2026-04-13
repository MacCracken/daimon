# Contributing to Daimon

Thank you for your interest in contributing to Daimon! This document outlines how
to get involved.

## Getting Started

1. Fork the repository and clone your fork
2. Install [Cyrius](https://github.com/MacCracken/cyrius) (v4.1.0+)
3. Run `cyrius check` to verify your environment

## Development Workflow

```bash
cyrius build src/main.cyr build/daimon   # Build
cyrius check                              # Format + lint + test + build
cyrius test tests/test.sh                 # Run tests
cyrius bench benches/bench.bcyr           # Run benchmarks
```

## Pull Requests

- Keep PRs focused — one feature or fix per PR
- Add tests for new logic
- Run `cyrius check` before submitting
- Update `CHANGELOG.md` under an `[Unreleased]` heading

## Code Style

- Follow existing patterns in the codebase
- Use `Result`/`Option` tagged unions for error handling — avoid panics
- Use accessor functions for struct fields
- Keep functions focused and small

## Adding a New Module

1. Add the module code to `src/main.cyr` (or a separate file included from it)
2. Add tests in `tests/`
3. Add benchmarks in `benches/`

## Reporting Issues

Open an issue on [GitHub](https://github.com/MacCracken/daimon/issues) with:
- What you expected to happen
- What actually happened
- Steps to reproduce
- Cyrius version (`cyrius --version`)

## License

By contributing, you agree that your contributions will be licensed under
GPL-3.0-only, consistent with the project license.
