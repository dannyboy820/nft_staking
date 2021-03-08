#!/bin/bash
set -o errexit -o nounset -o pipefail
command -v shellcheck > /dev/null && shellcheck "$0"

# Iterates over all example projects and publishes them. This updates lockfiles
# and provides a quick sanity check.
# This script is for development purposes only. In the CI, each example project
# is configured separately.

for example in ./contracts/*/; do
  echo "Publishing $example ..."
  (
      cd "$example"
      cargo publish
  )
done
