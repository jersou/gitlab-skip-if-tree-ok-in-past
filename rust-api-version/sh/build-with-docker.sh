#!/usr/bin/env sh

set -o errexit
dir_path=$(dirname "$0")
cd "$dir_path/build-with-docker"

docker compose build --progress plain
docker compose up
docker compose down
