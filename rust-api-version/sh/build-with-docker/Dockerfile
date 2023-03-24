FROM debian:buster-slim

RUN apt update && apt upgrade -y && apt install -y upx curl build-essential musl-tools
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > /tmp/rustup-init.sh
RUN sh /tmp/rustup-init.sh -y -v
ENV PATH=/root/.cargo/bin:$PATH
RUN rustup +nightly target add x86_64-unknown-linux-musl
RUN rustup component add rust-src --toolchain nightly-x86_64-unknown-linux-gnu

COPY . /src
WORKDIR /src

RUN sh/build.sh

RUN ls -alh /src/skip-if-tree-ok-in-past
