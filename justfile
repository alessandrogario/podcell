#
# Copyright (c) 2025-present, Alessandro Gario
# All rights reserved.
#
# This source code is licensed in accordance with the terms specified in
# the LICENSE file found in the root directory of this source tree.
#

default:
    @just --list

build:
    cargo build --release --target x86_64-unknown-linux-musl

check:
    cargo check --all-targets --all-features --examples --tests --workspace
    cargo clippy --all-targets --all-features --examples -- -D warnings
    cargo fmt --check

test:
    cargo test

format:
    cargo fmt
