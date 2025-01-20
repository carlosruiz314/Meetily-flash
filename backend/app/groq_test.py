from groq import Groq
from dotenv import load_dotenv
import os

load_dotenv()


client = Groq(api_key=os.getenv('GROQ_API_KEY'))
completion = client.chat.completions.create(
    model="llama-3.3-70b-versatile",
     messages=[
        {
            "role": "system",
            "content": "You area a helpful dude"
        }
    ],
    temperature=1,
    max_completion_tokens=1024,
    top_p=1,
    stream=True,
    stop=None,
)

for chunk in completion:
    print(chunk.choices[0].delta.content or "", end="")
