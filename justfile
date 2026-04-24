set shell := ["sh", "-cu"]

example_dir := "examples/ferrisium_demo"
minimal_globe_dir := "examples/minimal_globe"
minimal_map_dir := "examples/minimal_map"
kernel_dir := "examples/ferrisium_demo/assets/kernels"
wasm_dev_profile := "wasm-dev"
wasm_profile := "wasm-release"

default:
    @just --list

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt-check

lint:
    cargo lint

lint-wasm:
    cargo lint-wasm

check-wasm:
    cargo check-wasm

doc:
    cargo doc --workspace --no-deps

test:
    cargo test --workspace

test-core:
    cargo test-core

test-doc:
    cargo test-doc

quality:
    cargo fmt-check
    cargo lint
    cargo lint-wasm
    cargo check-wasm
    cargo test --workspace
    cargo doc --workspace --no-deps

clean:
    rm -rf target dist examples/*/dist
    find {{kernel_dir}} -mindepth 1 ! -name README.md -exec rm -rf {} +

web:
    cd {{example_dir}} && env -u NO_COLOR trunk serve --cargo-profile {{wasm_dev_profile}} --address 127.0.0.1 --port 8081 --disable-address-lookup

web-demo: web

web-map:
    cd {{minimal_map_dir}} && env -u NO_COLOR trunk serve --cargo-profile {{wasm_dev_profile}} --address 127.0.0.1 --port 8081 --disable-address-lookup

web-globe:
    cd {{minimal_globe_dir}} && env -u NO_COLOR trunk serve --cargo-profile {{wasm_dev_profile}} --address 127.0.0.1 --port 8081 --disable-address-lookup

web-release:
    cd {{example_dir}} && env -u NO_COLOR trunk serve --release --cargo-profile {{wasm_profile}} --address 127.0.0.1 --port 8081 --disable-address-lookup

web-demo-release: web-release

web-map-release:
    cd {{minimal_map_dir}} && env -u NO_COLOR trunk serve --release --cargo-profile {{wasm_profile}} --address 127.0.0.1 --port 8081 --disable-address-lookup

web-globe-release:
    cd {{minimal_globe_dir}} && env -u NO_COLOR trunk serve --release --cargo-profile {{wasm_profile}} --address 127.0.0.1 --port 8081 --disable-address-lookup

web-build:
    cd {{example_dir}} && env -u NO_COLOR -u RUST_LOG trunk build --cargo-profile {{wasm_dev_profile}}

web-build-map:
    cd {{minimal_map_dir}} && env -u NO_COLOR -u RUST_LOG trunk build --cargo-profile {{wasm_dev_profile}}

web-build-globe:
    cd {{minimal_globe_dir}} && env -u NO_COLOR -u RUST_LOG trunk build --cargo-profile {{wasm_dev_profile}}

web-once: web-build
    node scripts/static_no_cache_server.mjs --directory {{example_dir}}/dist --host 127.0.0.1 --port 8081

kernels:
    scripts/fetch_anise_kernels.sh

web-inspect *args: web-build
    node scripts/web_inspect.mjs {{args}}

web-test *args: web-build
    npm run web:test -- {{args}}

web-test-headed *args: web-build
    npm run web:test:headed -- {{args}}

web-test-record *args: web-build
    npm run web:test:record -- {{args}}

web-test-record-release *args: web-build-release
    npm run web:test:record:perf -- {{args}}

web-test-scenarios *args: web-build
    npm run web:test:scenarios -- {{args}}

web-test-scenarios-release *args: web-build-release
    npm run web:test:scenarios:perf -- {{args}}

web-build-release:
    cd {{example_dir}} && env -u NO_COLOR -u RUST_LOG trunk build --release --cargo-profile {{wasm_profile}} --minify=false

web-build-map-release:
    cd {{minimal_map_dir}} && env -u NO_COLOR -u RUST_LOG trunk build --release --cargo-profile {{wasm_profile}} --minify=false

web-build-globe-release:
    cd {{minimal_globe_dir}} && env -u NO_COLOR -u RUST_LOG trunk build --release --cargo-profile {{wasm_profile}} --minify=false

web-release-once: web-build-release
    node scripts/static_no_cache_server.mjs --directory {{example_dir}}/dist --host 127.0.0.1 --port 8081
