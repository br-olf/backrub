#!/bin/sh

mkdir -p target/doc
cp -a ~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/share/doc/rust/html/* target/doc/
cargo doc
