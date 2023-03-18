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
cargo +nightly build -Z build-std=std,panic_abort  --release --target x86_64-unknown-linux-musl -Z build-std-features=panic_immediate_abort --features vendored
cp target/x86_64-unknown-linux-musl/release/gitlab-skip-if-tree-ok-in-past-rust-api-version ./skip-if-tree-ok-in-past
upx --best --lzma ./skip-if-tree-ok-in-past

! ldd skip-if-tree-ok-in-past

##### cargo bloat ↓
# to list crates/fn size : remove `strip = true` and :

# $ cargo bloat --release --crates
#    File  .text     Size Crate
#    7.7%  27.2% 270.0KiB std
#    6.5%  22.9% 226.9KiB [Unknown]
#    4.2%  14.7% 145.9KiB libgit2_sys
#    2.2%   7.7%  76.2KiB hyper
#    1.8%   6.3%  62.3KiB zstd_sys
#    1.7%   6.0%  59.7KiB gitlab_skip_if_tree_ok_in_past_rust_api_version
#    1.0%   3.6%  35.8KiB tokio
#    0.6%   2.2%  21.8KiB miniz_oxide
#    0.6%   2.0%  20.1KiB http
#    0.5%   1.7%  16.7KiB futures_util
#    0.3%   1.2%  11.6KiB bzip2_sys
#    0.2%   0.8%   8.3KiB serde_json
#    0.1%   0.5%   5.0KiB sha1
#    0.1%   0.5%   4.9KiB bytes
#    0.1%   0.4%   4.3KiB rand
#    0.1%   0.4%   4.1KiB zip
#    0.1%   0.2%   2.4KiB serde
#    0.1%   0.2%   2.3KiB git2
#    0.0%   0.2%   1.6KiB crc32fast
#    0.0%   0.1%     994B flate2
#    0.1%   0.5%   4.7KiB And 25 more crates. Use -n N to show more.
#   28.5% 100.0% 992.7KiB .text section size, the file size is 3.4MiB

# $ cargo bloat --release -n 10
#    File  .text     Size                                           Crate Name
#    0.6%   2.2%  22.2KiB                                             std addr2line::ResDwarf<R>::parse
#    0.6%   2.2%  21.4KiB                                     libgit2_sys pcre_exec
#    0.6%   1.9%  19.2KiB gitlab_skip_if_tree_ok_in_past_rust_api_version gitlab_skip_if_tree_ok_in_past_rust_api_version::artifact::extract_artifacts::{{closure}}
#    0.5%   1.9%  18.4KiB                                             std std::backtrace_rs::symbolize::gimli::resolve::{{closure}}
#    0.5%   1.6%  16.0KiB                                       [Unknown] compile_regex
#    0.4%   1.6%  15.7KiB                                           hyper hyper::proto::h1::dispatch::Dispatcher<D,Bs,I,T>::poll_read
#    0.4%   1.4%  13.7KiB gitlab_skip_if_tree_ok_in_past_rust_api_version gitlab_skip_if_tree_ok_in_past_rust_api_version::process::find_last_job_ok::{{closure}}
#    0.4%   1.2%  12.4KiB gitlab_skip_if_tree_ok_in_past_rust_api_version gitlab_skip_if_tree_ok_in_past_rust_api_version::main::{{closure}}
#    0.3%   1.2%  12.2KiB                                           hyper hyper::client::client::Client<C,B>::retryably_send_request::{{closure}}
#    0.3%   1.1%  10.6KiB                                             std addr2line::ResUnit<R>::parse_lines
#   23.6%  83.0% 823.8KiB                                                 And 3464 smaller methods. Use -n N to show more.
#   28.5% 100.0% 992.7KiB                                                 .text section size, the file size is 3.4MiB
