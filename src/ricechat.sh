#!/usr/bin/env sh

# Llama server running
API_URL="${API_URL:-http://127.0.0.1:8080}"

if [ "$#" -ne 1  ]; then
    exit 1
fi

PROMPT="$1"
PERSONALITY="patriot.sh"

# This will source CHAT and INSTRUCTION
source $PWD/$PERSONALITY

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
