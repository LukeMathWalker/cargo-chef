#!/bin/bash
set -ex pipefail
# Set the environment variables needed to get coverage.
source <(cargo llvm-cov show-env --sh)
# Remove artifacts that may affect the coverage results.
cargo llvm-cov clean --workspace
cargo build # Build `cargo-chef` binary with instrumentation.
cargo test # Run tests with instrumentation.
cargo llvm-cov report --json --output-path=coverage.json
