#!/usr/bin/env bash

set -o errexit
dir_path=$(dirname "$0")
cd "$dir_path/.."

# to generate a small binary : https://github.com/johnthagen/min-sized-rust

##### gnu build ↓
# cargo +nightly build -Z build-std=std,panic_abort  --release --target x86_64-unknown-linux-gnu -Z build-std-features=panic_immediate_abort
# cp target/x86_64-unknown-linux-gnu/release/gitlab-skip-if-tree-ok-in-past-rust-api-version ./skip-if-tree-ok-in-past
# upx --best --lzma ./skip-if-tree-ok-in-past

##### musl build ↓
# rustup +nightly target add x86_64-unknown-linux-musl
# rustup target add x86_64-unknown-linux-musl
cargo +nightly build \
                   --release  \
                   --target x86_64-unknown-linux-musl  \
                   -Z build-std=std,panic_abort  \
                   -Z build-std-features=panic_immediate_abort
cp target/x86_64-unknown-linux-musl/release/gitlab-skip-if-tree-ok-in-past-rust-api-version ./skip-if-tree-ok-in-past
upx --best --lzma ./skip-if-tree-ok-in-past

! ldd skip-if-tree-ok-in-past



##### cargo bloat ↓
# to list crates/fn size : remove `strip = true` and :

# $ cargo bloat --release --crates
#    File  .text     Size Crate
#    6.3%  23.1% 306.0KiB [Unknown]
#    5.9%  21.6% 285.8KiB std
#    2.9%  10.6% 141.0KiB libgit2_sys
#    2.2%   8.0% 106.3KiB rustls
#    1.9%   7.1%  93.9KiB ring
#    1.6%   5.9%  77.5KiB hyper
#    1.3%   4.7%  61.6KiB gitlab_skip_if_tree_ok_in_past_rust_api_version
#    1.2%   4.3%  56.5KiB zstd_sys
#    0.7%   2.7%  36.2KiB tokio
#    0.4%   1.6%  21.8KiB miniz_oxide
#    0.4%   1.5%  20.2KiB http
#    0.4%   1.3%  17.4KiB futures_util
#    0.2%   0.9%  11.6KiB bzip2_sys
#    0.2%   0.8%  10.1KiB hyper_rustls
#    0.2%   0.7%   8.9KiB tokio_rustls
#    0.2%   0.7%   8.7KiB serde_json
#    0.2%   0.6%   7.9KiB webpki
#    0.2%   0.6%   7.7KiB anyhow
#    0.1%   0.4%   5.0KiB sha1
#    0.1%   0.4%   4.9KiB bytes
#    0.4%   1.6%  21.8KiB And 31 more crates. Use -n N to show more.
#   27.2% 100.0%   1.3MiB .text section size, the file size is 4.8MiB

# $ cargo bloat --release -n 10
#    File  .text    Size                                           Crate Name
#    0.5%   1.7% 22.2KiB                                             std addr2line::ResDwarf<R>::parse
#    0.4%   1.6% 21.4KiB                                     libgit2_sys pcre_exec
#    0.4%   1.4% 19.0KiB gitlab_skip_if_tree_ok_in_past_rust_api_version gitlab_skip_if_tree_ok_in_past_rust_api_version::artifact::extract_artifacts::{{closure}}
#    0.4%   1.4% 18.4KiB                                             std std::backtrace_rs::symbolize::gimli::resolve::{{closure}}
#    0.3%   1.2% 16.0KiB                                       [Unknown] compile_regex
#    0.3%   1.2% 15.7KiB                                           hyper hyper::proto::h1::dispatch::Dispatcher<D,Bs,I,T>::poll_read
#    0.3%   1.0% 13.8KiB gitlab_skip_if_tree_ok_in_past_rust_api_version gitlab_skip_if_tree_ok_in_past_rust_api_version::main::{{closure}}
#    0.3%   1.0% 13.5KiB gitlab_skip_if_tree_ok_in_past_rust_api_version gitlab_skip_if_tree_ok_in_past_rust_api_version::find_last_job_ok::find_last_job_ok::{{closure}}
#    0.2%   0.9% 12.1KiB                                          rustls rustls::msgs::handshake::HandshakeMessagePayload::read_version
#    0.2%   0.9% 11.9KiB                                       [Unknown] chacha20_poly1305_seal_avx2
#   23.5%  86.6%  1.1MiB                                                 And 4572 smaller methods. Use -n N to show more.
#   27.2% 100.0%  1.3MiB                                                 .text section size, the file size is 4.8MiB
