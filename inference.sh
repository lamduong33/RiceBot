#!/usr/bin/env sh

# Assuming that llama.cpp's "main" binary is called "llama" here.
# Any model is in .gguf extension
./llama -m capybarahermes-2.5-mistral-7b.Q4_K_M.gguf --color -c 32768 --temp 0.7 --repeat_penalty 1.1 -n -1 -p "<|im_start|>system\n{system_message}<|im_end|>\n<|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant"
