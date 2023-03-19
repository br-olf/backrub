#!/bin/bash

mkdir -p target/doc
cargo doc
cp -a $(dirname $(rustup doc --path))/* target/doc/
