import sys
import os

# LLM Magic
from langchain_community.llms import LlamaCpp
from langchain_core.messages import HumanMessage
from langchain_core.messages import AIMessage
from langchain.prompts import ChatPromptTemplate, PromptTemplate
from langchain.chains import LLMChain

def find_root_dir() -> str:
    """ Find the root directory of the project. """
    current_dir = os.path.abspath(os.getcwd())
    while True:
        if '.git' in os.listdir(current_dir):
            return current_dir
        current_dir = os.path.dirname(current_dir)
        if current_dir == os.path.dirname(current_dir):
            break
    return ""

def load_file_content(file_path:str ) -> str:
    """
    Get the content of the file and load it onto a string.
    """
    try:
        with open(file_path, 'r') as file:
            file_content = file.read()
        return file_content
    except FileNotFoundError:
        print("File not found.")
        return ""

# Example usage:

def main(prompt):

    root_dir = find_root_dir()
    capybara_path = root_dir + "/capybarahermes-2.5-mistral-7b.Q4_K_M.gguf"
    llama_path = root_dir + "/llama-2-7b-chat.Q4_K_M.gguf"
    identity = load_file_content(root_dir + "identity.txt")

    # Load the model
    llm = LlamaCpp(
        model_path=llama_path,
        n_gpu_layers=40,
        n_batch=512,
        verbose=False,
    )
    template = ChatPromptTemplate.from_messages(
        [
            (
                "system",
                "You are a helpful assistant"
            )
        ]
    )
    answer = llm.invoke(
        [
            HumanMessage(
                content=prompt
            ),        AIMessage(content="J'adore la programmation."),
        HumanMessage(content="What did you just say?"),
        ]
    )
    print(answer)

if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(1) # NOTE: Make sure to catch this as error code
    prompt = sys.argv[1]
    main(prompt)
    sys.exit(0)
