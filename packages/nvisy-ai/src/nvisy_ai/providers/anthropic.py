"""Anthropic completion provider."""

import base64
from anthropic import Anthropic
from .base import CompletionClient


class AnthropicClient(CompletionClient):
    """Anthropic-based completion client."""

    def __init__(self, api_key: str, model: str = "claude-sonnet-4-5-20250929"):
        self._client = Anthropic(api_key=api_key)
        self._model = model

    async def complete(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.0,
    ) -> str:
        response = self._client.messages.create(
            model=self._model,
            max_tokens=4096,
            temperature=temperature,
            system=system_prompt,
            messages=[{"role": "user", "content": user_prompt}],
        )
        return response.content[0].text if response.content else ""

    async def complete_with_image(
        self,
        system_prompt: str,
        image_bytes: bytes,
        mime_type: str,
        user_prompt: str,
        temperature: float = 0.0,
    ) -> str:
        b64 = base64.b64encode(image_bytes).decode("utf-8")
        response = self._client.messages.create(
            model=self._model,
            max_tokens=4096,
            temperature=temperature,
            system=system_prompt,
            messages=[
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": mime_type,
                                "data": b64,
                            },
                        },
                        {"type": "text", "text": user_prompt},
                    ],
                },
            ],
        )
        return response.content[0].text if response.content else ""
