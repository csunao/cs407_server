#!/bin/bash

LD_LIBRARY_PATH=target/debug/
cargo build
gcc -std=c11 src/test.c -L./target/debug -lcs407_server -o main
