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
rm -f ./skip-if-tree-ok-in-past
upx --best --ultra-brute -o ./skip-if-tree-ok-in-past target/x86_64-unknown-linux-musl/release/gitlab-skip-if-tree-ok-in-past-rust-api-version

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


# https://github.com/google/bloaty
# $ bloaty gitlab-skip-if-tree-ok-in-past-rust-api-version  -d compileunits
#     FILE SIZE        VM SIZE
#  --------------  --------------
#   20.4%   537Ki  34.1%   537Ki    [section .text]/crt/rcrt1.c
#   18.0%   474Ki  19.0%   299Ki    ../src_musl/crt/rcrt1.cext]
#   14.2%   373Ki   9.5%   149Ki    [268 Others]]
#    8.5%   224Ki   2.8%  44.5Ki    crypto/curve25519/curve25519.ce25519/curve25519.c
#    6.1%   160Ki   9.5%   149Ki    crypto/fipsmodule/ec/ecp_nistz256.cmodule/ec/ecp_nistz256.c
#    4.7%   123Ki   0.0%       0    [section .strtab]trtab]
#    4.6%   122Ki   7.7%   122Ki    [section .rodata]odata]
#    4.1%   108Ki   0.4%  6.72Ki    crypto/poly1305/poly1305_vec.c1305/poly1305_vec.c
#    3.9%   101Ki   3.8%  60.3Ki    ../src_musl/src/exit/exit.c/src/exit/exit.c
#    3.0%  79.0Ki   0.3%  4.63Ki    crypto/fipsmodule/aes/aes_nohw.cmodule/aes/aes_nohw.c
#    2.2%  58.3Ki   0.0%       0    [section .symtab]ymtab]
#    1.9%  50.5Ki   3.2%  50.5Ki    [section .eh_frame]h_frame]
#    1.8%  48.3Ki   2.5%  39.8Ki    /home/jer/.cargo/registry/src/index.crates.io-6f17d22bba15001f/ring-0.16.20/pregenerated/chacha20_poly1305_x86_64-elf.Sgistry/src/index.crates.io-6f17d22bba15001f/ring-0.16.20/pregenerated/chacha20_poly1305_x86_64-elf.S
#    1.2%  32.8Ki   0.3%  4.02Ki    crypto/fipsmodule/ec/gfp_p384.cata.rel.ro]
#    0.0%       0   2.1%  32.4Ki    [section .data.rel.ro]module/ec/gfp_p384.c
#    1.1%  28.8Ki   0.1%  1.91Ki    crypto/limbs/limbs.cs/limbs.c
#    0.9%  24.8Ki   1.2%  19.5Ki    ../src_musl/src/stdio/vfprintf.c/src/stdio/vfprintf.c
#    0.9%  23.6Ki   1.4%  22.3Ki    ../src_musl/src/passwd/nscd_query.c/src/passwd/nscd_query.c
#    0.8%  22.1Ki   1.1%  17.1Ki    /home/jer/.cargo/registry/src/index.crates.io-6f17d22bba15001f/ring-0.16.20/pregenerated/p256-x86_64-asm-elf.Sgistry/src/index.crates.io-6f17d22bba15001f/ring-0.16.20/pregenerated/p256-x86_64-asm-elf.S
#    0.8%  19.9Ki   0.0%       0    [section .debug_frame]ebug_frame]
#    0.7%  17.4Ki   0.9%  13.6Ki    /home/jer/.cargo/registry/src/index.crates.io-6f17d22bba15001f/ring-0.16.20/pregenerated/x86_64-mont5-elf.Sgistry/src/index.crates.io-6f17d22bba15001f/ring-0.16.20/pregenerated/x86_64-mont5-elf.S
#  100.0%  2.57Mi 100.0%  1.54Mi    TOTAL
