#!/usr/bin/env bash

set -o errexit
dir_path=$(dirname "$0")
cd "$dir_path/.."

RUST_TEST_NOCAPTURE=1 cargo test -- --test-threads=1
