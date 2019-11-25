#!/bin/bash
set -o errexit -o nounset -o pipefail
command -v shellcheck > /dev/null && shellcheck "$0"

# Iterates over all example projects, builds and tests them.
# This script is for development purposes only. In the CI, each example project
# is configured separately.

export RUST_BACKTRACE=1

for example in ./*; do
  if [[ -d "$example" ]]; then
    echo "Building and testing $example ..."

    (
        cd "$example"

        # Build wasm binaries
        cargo wasm --locked

        # Run all tests (rust unit tests, vm integration tests)
        cargo test --features backtraces --locked
    )
  fi
done
