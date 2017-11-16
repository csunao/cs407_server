#!/bin/bash

cargo build
gcc -std=c11 src/test.c -L./target/debug -lcs407_server -o main
LD_LIBRARY_PATH=target/debug/ ./main
