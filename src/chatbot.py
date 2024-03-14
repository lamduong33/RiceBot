#!/usr/bin/env python3

import panel as pn
from ctransformers import AutoModelForCausalLM

# Some const global variables
repo_id = "TheBloke/Mistral-7B-Instruct-v0.1-GGUF"
model_file = "mistral-7b-instruct-v0.1.Q4_K_M.gguf"

# Start panel
pn.extension()

async def callback(contents: str, user: str, instance: pn.chat.ChatInterface):

    if "mistral" not in llms:
        instance.placeholder_text = "Downloading model; please wait..."
        # gpu_layers=0 for running on PCU
        llms["mistral"] = AutoModelForCausalLM.from_pretrained(
            repo_id,
            model_file=model_file,
            gpu_layers=0,
        )

    llm = llms["mistral"]
    # Streaming determines if each token will be printed out each time or not.
    response = llm(contents, stream=True, max_new_tokens=1000)
    message = ""

    for token in response:
        message += token
        yield message

llms = {}

chat_interface = pn.chat.ChatInterface(callback=callback, callback_user="Mistral")

chat_interface.send(
    "Send a message to get a reply from Mistral!", user="System", respond=False
)

chat_interface.servable()
