"""Google Gemini completion provider."""

import google.generativeai as genai
from .base import CompletionClient


class GeminiClient(CompletionClient):
    """Google Gemini-based completion client."""

    def __init__(self, api_key: str, model: str = "gemini-1.5-pro"):
        genai.configure(api_key=api_key)
        self._model = genai.GenerativeModel(model)

    async def complete(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.0,
    ) -> str:
        response = self._model.generate_content(
            f"{system_prompt}\n\n{user_prompt}",
            generation_config=genai.types.GenerationConfig(temperature=temperature),
        )
        return response.text or ""

    async def complete_with_image(
        self,
        system_prompt: str,
        image_bytes: bytes,
        mime_type: str,
        user_prompt: str,
        temperature: float = 0.0,
    ) -> str:
        image_part = {"mime_type": mime_type, "data": image_bytes}
        response = self._model.generate_content(
            [f"{system_prompt}\n\n{user_prompt}", image_part],
            generation_config=genai.types.GenerationConfig(temperature=temperature),
        )
        return response.text or ""
