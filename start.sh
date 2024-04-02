#!/bin/bash

# Check if an argument is provided
if [ $# -ne 1 ]; then
    echo "Usage: $0 <model>"
    exit 1
fi

# Accessing the argument
MODEL=$1

./server -m "$MODEL" &
./target/debug/rice_bot                                                                                                                                                                                    10:06:24
