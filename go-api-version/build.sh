#!/usr/bin/env bash

set -o errexit

export CGO_ENABLED=0 
export GOOS=linux
export GOARCH=amd64

go build -a -tags netgo -ldflags="-s -w -extldflags "-static"" -o skip-if-tree-ok-in-past skipIfTreeOkInPast.go

upx --brute skip-if-tree-ok-in-past



