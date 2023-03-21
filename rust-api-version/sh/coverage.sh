#!/usr/bin/env bash

set -o errexit
dir_path=$(dirname "$0")
cd "$dir_path/.."

SKIP_CI_VERBOSE=true \
    RUST_TEST_NOCAPTURE=1 \
      cargo +nightly tarpaulin \
          --out html \
          --output-dir target \
              -- --test-threads=1

xdg-open target/tarpaulin-report.html

