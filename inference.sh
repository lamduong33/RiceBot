#!/usr/bin/env sh

# No arguments, then exit
if [ $# -ne 1 ]; then
    echo "Usage: $0 <argument>"
    exit 1
fi

CONTEXT_SIZE=512
MODEL=capybarahermes-2.5-mistral-7b.Q4_K_M.gguf
PROMPT=$1 # prompt is whatever that's passed via argument of the script
TEMPERATURE=0.3

# Assuming that llama.cpp's "main" binary is called "llama" here.
# Any model is in .gguf extension
./llama -m $MODEL -c $CONTEXT_SIZE --temp $TEMPERATURE \
    --repeat_penalty 1.1 -n -1 -p "$PROMPT" \
    2>/dev/null
