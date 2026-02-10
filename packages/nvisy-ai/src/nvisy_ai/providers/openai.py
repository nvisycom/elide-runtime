"""OpenAI completion provider."""

import base64
from openai import OpenAI
from .base import CompletionClient


class OpenAIClient(CompletionClient):
    """OpenAI-based completion client."""

    def __init__(self, api_key: str, model: str = "gpt-4"):
        self._client = OpenAI(api_key=api_key)
        self._model = model

    async def complete(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.0,
    ) -> str:
        response = self._client.chat.completions.create(
            model=self._model,
            temperature=temperature,
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt},
            ],
        )
        return response.choices[0].message.content or ""

    async def complete_with_image(
        self,
        system_prompt: str,
        image_bytes: bytes,
        mime_type: str,
        user_prompt: str,
        temperature: float = 0.0,
    ) -> str:
        b64 = base64.b64encode(image_bytes).decode("utf-8")
        response = self._client.chat.completions.create(
            model=self._model,
            temperature=temperature,
            messages=[
                {"role": "system", "content": system_prompt},
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": f"data:{mime_type};base64,{b64}",
                            },
                        },
                        {"type": "text", "text": user_prompt},
                    ],
                },
            ],
        )
        return response.choices[0].message.content or ""
