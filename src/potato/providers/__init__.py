# -*- coding: utf-8 -*-
"""Provider management — models, registry + persistent store.

The provider models are deliberately cheap to import.  The registry manager
also imports every concrete provider implementation, and some of those
implementations pull in large third-party SDKs.  Keep that manager behind a
module-level lazy attribute so importing a model schema does not eagerly load
all provider SDKs.
"""

from typing import TYPE_CHECKING, Any

from .provider import ModelInfo, Provider, ProviderInfo

if TYPE_CHECKING:
    from .provider_manager import ProviderManager


def __getattr__(name: str) -> Any:
    """Load the provider registry only when it is actually requested."""
    if name == "ProviderManager":
        from .provider_manager import ProviderManager

        return ProviderManager
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


__all__ = [
    "ModelInfo",
    "Provider",
    "ProviderManager",
    "ProviderInfo",
]
