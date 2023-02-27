#!/usr/bin/env bash

docker run                   \
        --rm                 \
        -it                  \
        -v "$(pwd):/src"     \
        --workdir /src       \
        golang:1.20-bullseye \
           bash -c           \
              "apt update && apt install -y upx-ucl && ./build.sh"
