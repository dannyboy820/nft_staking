#!/bin/bash
set -o errexit -o nounset -o pipefail
command -v shellcheck > /dev/null && shellcheck "$0"

# This script is mainly for CI, but should work on a dev machine as well.
# Iterates over all example projects and tests them

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
