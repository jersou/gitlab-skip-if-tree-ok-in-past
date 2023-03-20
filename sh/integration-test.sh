#!/usr/bin/env bash

set -o errexit
dir_path=$(dirname "$0")
cd "$dir_path/../test/integration"

docker compose down -v
docker compose build
docker compose up -d gitlab-fake-api

docker compose up --exit-code-from=rust-test-scratch-ko                     rust-test-scratch-ko && exit 1
docker compose up --exit-code-from=rust-test-scratch-ok                     rust-test-scratch-ok
docker compose up --exit-code-from=rust-test-ubuntu--tree-found-in-job      rust-test-ubuntu--tree-found-in-job
docker compose up --exit-code-from=rust-test-ubuntu--tree-not-found-in-job  rust-test-ubuntu--tree-not-found-in-job

docker compose down -v
