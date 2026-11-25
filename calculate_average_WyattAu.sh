#!/bin/sh
cargo build --release --manifest-path ./src/main/rust/WyattAu/Cargo.toml
time ./src/main/rust/WyattAu/target/release/wyattau measurements.txt
