#!/bin/bash
set -euo pipefail
cargo bench --bench benchmarks -- --output-format bencher 2>/dev/null | tee -a bench-history.csv
