"""NER detection functions called from Rust via PyO3."""

import json
from typing import Optional

from .prompts import NER_SYSTEM_PROMPT, NER_IMAGE_SYSTEM_PROMPT
from .providers.base import CompletionClient


def _get_client(provider: str, api_key: str, model: str) -> CompletionClient:
    """Create a completion client for the given provider."""
    if provider == "openai":
        from .providers.openai import OpenAIClient
        return OpenAIClient(api_key=api_key, model=model)
    elif provider == "anthropic":
        from .providers.anthropic import AnthropicClient
        return AnthropicClient(api_key=api_key, model=model)
    elif provider == "gemini":
        from .providers.gemini import GeminiClient
        return GeminiClient(api_key=api_key, model=model)
    else:
        raise ValueError(f"Unknown provider: {provider}")


def _parse_entities(response_text: str) -> list[dict]:
    """Parse the JSON response from the LLM into entity dicts."""
    text = response_text.strip()
    # Strip markdown code fences if present
    if text.startswith("```"):
        lines = text.split("\n")
        lines = [l for l in lines if not l.startswith("```")]
        text = "\n".join(lines)

    try:
        entities = json.loads(text)
    except json.JSONDecodeError:
        return []

    if not isinstance(entities, list):
        return []

    return entities


def detect_ner(
    text: str,
    entity_types: Optional[list[str]] = None,
    confidence_threshold: float = 0.5,
    temperature: float = 0.0,
    api_key: str = "",
    model: str = "gpt-4",
    provider: str = "openai",
) -> list[dict]:
    """Detect named entities in text using an LLM.

    Called from Rust via PyO3.

    Args:
        text: The text to analyze.
        entity_types: Optional list of entity types to detect.
        confidence_threshold: Minimum confidence to include.
        temperature: LLM temperature parameter.
        api_key: API key for the provider.
        model: Model name to use.
        provider: Provider name ("openai", "anthropic", "gemini").

    Returns:
        List of entity dicts with keys: category, entity_type, value,
        confidence, start_offset, end_offset.
    """
    import asyncio

    client = _get_client(provider, api_key, model)

    user_prompt = f"Analyze the following text for sensitive data:\n\n{text}"
    if entity_types:
        user_prompt += f"\n\nOnly detect these entity types: {', '.join(entity_types)}"

    loop = asyncio.new_event_loop()
    try:
        response = loop.run_until_complete(
            client.complete(NER_SYSTEM_PROMPT, user_prompt, temperature)
        )
    finally:
        loop.close()

    entities = _parse_entities(response)

    # Filter by confidence threshold
    return [
        e for e in entities
        if e.get("confidence", 0) >= confidence_threshold
    ]


def detect_ner_image(
    image_bytes: bytes,
    mime_type: str,
    entity_types: Optional[list[str]] = None,
    confidence_threshold: float = 0.5,
    temperature: float = 0.0,
    api_key: str = "",
    model: str = "gpt-4",
    provider: str = "openai",
) -> list[dict]:
    """Detect named entities in an image using a multimodal LLM.

    Called from Rust via PyO3.

    Args:
        image_bytes: Raw image bytes.
        mime_type: MIME type of the image.
        entity_types: Optional list of entity types to detect.
        confidence_threshold: Minimum confidence to include.
        api_key: API key for the provider.
        model: Model name to use.
        provider: Provider name ("openai", "anthropic", "gemini").

    Returns:
        List of entity dicts.
    """
    import asyncio

    client = _get_client(provider, api_key, model)

    user_prompt = "Analyze this image for any visible sensitive data."
    if entity_types:
        user_prompt += f"\n\nOnly detect these entity types: {', '.join(entity_types)}"

    loop = asyncio.new_event_loop()
    try:
        response = loop.run_until_complete(
            client.complete_with_image(
                NER_IMAGE_SYSTEM_PROMPT, image_bytes, mime_type, user_prompt, temperature
            )
        )
    finally:
        loop.close()

    entities = _parse_entities(response)

    return [
        e for e in entities
        if e.get("confidence", 0) >= confidence_threshold
    ]
