set shell := ["nu", "-c"]

release:
    cargo build --release

install:
    cargo install --path .

build:
    just release
    just install