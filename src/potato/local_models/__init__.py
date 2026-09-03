# -*- coding: utf-8 -*-
"""Local model management and inference.

Submodules are imported lazily: ``manager``/``llamacpp`` pull in httpx and
the llama.cpp helpers, which the desktop backend should not pay for on the
startup path.
"""

from __future__ import annotations

_LAZY = {
    "LocalModelManager": ".manager",
    "LocalModelConfig": ".schemas",
    "ModelManager": ".model_manager",
    "LocalModelInfo": ".model_manager",
    "DownloadSource": ".model_manager",
    "LlamaCppBackend": ".llamacpp",
}


def __getattr__(name: str):
    module_name = _LAZY.get(name)
    if module_name is None:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
    from importlib import import_module

    return getattr(import_module(module_name, __name__), name)


__all__ = list(_LAZY)
