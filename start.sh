#!/bin/bash

# Check if an argument is provided
if [ $# -ne 2 ]; then
    echo "Usage: $0 <token_file> <model>"
    exit 1
fi

# Accessing the argument
TOKEN_FILE=$1
MODEL=$2

./server -m "$MODEL" &
./target/debug/rice_bot "$TOKEN_FILE" "$MODEL"
