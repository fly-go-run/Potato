# -*- coding: utf-8 -*-
"""Lightweight persisted local-model configuration schemas."""

from __future__ import annotations

from pydantic import BaseModel, Field


class LocalModelConfig(BaseModel):
    """Persistent local runtime settings for embedded llama.cpp."""

    max_context_length: int = Field(
        default=65536,
        description="Maximum context length passed to llama.cpp on startup.",
        ge=32768,
    )
    port: int | None = Field(
        default=None,
        description=(
            "Optional fixed port for llama.cpp startup. Null means auto."
        ),
        ge=1,
        le=65535,
    )
