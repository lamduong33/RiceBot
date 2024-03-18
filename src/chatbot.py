import sys
import os

# LLM Magic
from langchain_community.llms import LlamaCpp
from langchain_core.messages import HumanMessage
from langchain.prompts import PromptTemplate
from langchain.chains import LLMChain

def find_root_dir():
    current_dir = os.path.abspath(os.getcwd())
    while True:
        if '.git' in os.listdir(current_dir):
            return current_dir
        current_dir = os.path.dirname(current_dir)
        if current_dir == os.path.dirname(current_dir):
            break
    return ""

def main(prompt):

    root_dir = find_root_dir()
    capybara_path = root_dir + "/capybarahermes-2.5-mistral-7b.Q4_K_M.gguf"
    llama_path = root_dir + "/llama-2-7b-chat.Q4_K_M.gguf"

    # Load the model
    llm = LlamaCpp(
        model_path=llama_path,
        n_gpu_layers=40,
        n_batch=512,
        verbose=False,
    )
    answer = llm.invoke(
        [
            HumanMessage(
                content=prompt
            )
        ]
    )
    print(answer)

if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(1) # NOTE: Make sure to catch this as error code
    prompt = sys.argv[1]
    main(prompt)
    sys.exit(0)
