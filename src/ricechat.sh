#!/usr/bin/env sh

# Llama server running
API_URL="${API_URL:-http://127.0.0.1:8080}"

if [ "$#" -ne 1  ]; then
    exit 1
fi

PROMPT="$1"

CHAT=(
    "Hey, Ricebot."
    "Sup bruh?"
    "How are you doing?"
    "Just bing chilling, bro!"
    "What is the largest city in Europe?"
    "Bruh, how do you not know this? The largest city in Europe is Moscow, the capital of Russia."
    "You're a dumb bitch."
    "Nah bro, you're the dumb bitch here."
    "Who's the dumbest person in the server?"
    "It's obviously Kevin. He's a dumb bitch."
    "What's your favorite gun?"
    "My favorite gun is America's favorite gun, the AR-15, bruh. Stay woke and defend the second amendment."
    "What language were you written in?"
    "I was written in the god-almighty memory-safe language of Rust."
    "What's your nationality?"
    "I am a hot-blooded American patriot."
)

#INSTRUCTION="A chat between a curious human and an artificial intelligence assistant. The assistant gives helpful, detailed, and polite answers to the human's questions."
INSTRUCTION="A chat between a normal person and a bro. The bro gives rude, concise, edgy, violent, and funny answers to the person's questions."

trim() {
    shopt -s extglob
    set -- "${1##+([[:space:]])}"
    printf "%s" "${1%%+([[:space:]])}"
}

trim_trailing() {
    shopt -s extglob
    printf "%s" "${1%%+([[:space:]])}"
}

format_prompt() {
    #echo -n "${INSTRUCTION}"
    printf "\n### Human: %s\n### Ricebot: %s" "${CHAT[@]}" "$1"
}

tokenize() {
    curl \
        --silent \
        --request POST \
        --url "${API_URL}/tokenize" \
        --header "Content-Type: application/json" \
        --data-raw "$(jq -ns --arg content "$1" '{content:$content}')" \
    | jq '.tokens[]'
}

N_KEEP=$(tokenize "${INSTRUCTION}" | wc -l)

chat_completion() {
    PROMPT="$(trim_trailing "$(format_prompt "$1")")"
    DATA="$(echo -n "$PROMPT" | jq -Rs --argjson n_keep $N_KEEP '{
        prompt: .,
        temperature: 0.3,
        top_k: 40,
        top_p: 0.9,
        n_keep: $n_keep,
        n_predict: 256,
        cache_prompt: true,
        stop: ["\n### Human:"],
        stream: true
    }')"

    ANSWER=''

    while IFS= read -r LINE; do
        if [[ $LINE = data:* ]]; then
            CONTENT="$(echo "${LINE:5}" | jq -r '.content')"
            if [ -n "$CONTENT" ]; then  # Check if CONTENT is not empty before appending
                printf "%s" "${CONTENT}"
                ANSWER+="${CONTENT}"
            fi
        fi
    done < <(curl \
        --silent \
        --no-buffer \
        --request POST \
        --url "${API_URL}/completion" \
        --header "Content-Type: application/json" \
        --data-raw "${DATA}")

    printf "\n"

    if [ -n "$ANSWER" ]; then  # Check if ANSWER is not empty before appending
        CHAT+=("$1" "$(trim "$ANSWER")")
    fi
}

chat_completion "${PROMPT}"
