#!/usr/bin/env bash

set -o errexit

docker build -t tmp-go-cache-version .
containerId=$(docker create tmp-go-cache-version)
docker cp "$containerId":/src/skip-if-tree-ok-in-past ./skip-if-tree-ok-in-past
docker rm "$containerId"
docker rmi tmp-go-cache-version
