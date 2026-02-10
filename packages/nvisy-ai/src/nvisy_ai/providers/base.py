"""Abstract base class for AI completion providers."""

from abc import ABC, abstractmethod
from typing import Any


class CompletionClient(ABC):
    """Abstract completion client for LLM providers."""

    @abstractmethod
    async def complete(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.0,
    ) -> str:
        """Send a completion request and return the response text."""
        ...

    @abstractmethod
    async def complete_with_image(
        self,
        system_prompt: str,
        image_bytes: bytes,
        mime_type: str,
        user_prompt: str,
        temperature: float = 0.0,
    ) -> str:
        """Send a multimodal completion request with an image."""
        ...
