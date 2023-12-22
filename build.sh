#!/usr/bin/env bash

set -eu

cargo build --bin glyfi-init-db
cargo run --bin glyfi-init-db
cargo build
