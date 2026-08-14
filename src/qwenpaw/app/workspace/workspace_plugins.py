# -*- coding: utf-8 -*-
"""Per-workspace pluggable layer.

Holds the three per-workspace registries that ``Runtime.run()``
reads each request:

* :class:`SlashCommandRegistry` — slash dispatch
* :class:`HookRegistry`         — 8-phase hook orchestration
* ``modes``                     — list of :class:`AgentMode` instances

Every field is **per-workspace** — no cross-workspace sharing. The
matching cross-workspace container is ``AppServiceManager`` and is strictly
limited to its three coordinators (see ``app/app_services/``).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING

from ...runtime.hooks import HookRegistry
from ...runtime.prompt_manager import PromptManager
from ...runtime.slash_command_registry import SlashCommandRegistry
from ...runtime.tool_registry import ToolRegistry
from ...runtime.registration import RegistrationHandle, Scope

if TYPE_CHECKING:
    from ...loop.gates import StopHandlerRegistration
    from ...modes.base import AgentMode
    from ...runtime.hooks import HookContext


@dataclass
class WorkspacePlugins:
    """Per-workspace pluggable registries."""

    slash_command_registry: SlashCommandRegistry = field(
        default_factory=SlashCommandRegistry,
    )
    hook_registry: HookRegistry = field(default_factory=HookRegistry)
    tool_registry: ToolRegistry = field(default_factory=ToolRegistry)
    prompt_manager: PromptManager = field(default_factory=PromptManager)
    modes: list["AgentMode"] = field(default_factory=list)
    stop_handlers: list["StopHandlerRegistration"] = field(
        default_factory=list,
    )
    scope: Scope = field(
        default_factory=lambda: Scope(tag="workspace"),
        repr=False,
    )
    _active_registration_scope: Scope | None = field(
        default=None,
        init=False,
        repr=False,
    )

    def _capture(self, handle: RegistrationHandle) -> RegistrationHandle:
        if self._active_registration_scope is not None:
            self._active_registration_scope.add(handle)
        return handle

    def register_slash_command(self, spec) -> RegistrationHandle:
        return self._capture(self.slash_command_registry.register(spec))

    def register_runtime_hook(self, hook) -> RegistrationHandle:
        return self._capture(self.hook_registry.register(hook))

    def register_tool(self, desc) -> RegistrationHandle:
        return self._capture(self.tool_registry.register(desc))

    def register_prompt_contributor(self, contributor) -> RegistrationHandle:
        return self._capture(self.prompt_manager.register(contributor))

    def register_fallback(self, handler) -> RegistrationHandle:
        return self._capture(
            self.slash_command_registry.register_fallback(handler),
        )

    def register_stop_handler(
        self,
        registration: "StopHandlerRegistration",
    ) -> RegistrationHandle:
        self.stop_handlers.append(registration)

        def _dispose() -> None:
            for index, registered in enumerate(self.stop_handlers):
                if registered is registration:
                    del self.stop_handlers[index]
                    break

        return self._capture(
            RegistrationHandle(
                _dispose,
                tag=f"stop-handler:{registration.name}",
            ),
        )

    def register_mode(
        self,
        mode: "AgentMode",
        workspace: object,
    ) -> RegistrationHandle:
        """Add ``mode`` and immediately run its ``setup(workspace)``.

        Duplicate names are rejected — collisions usually mean two
        bootstrap paths both think they own the mode and silently
        double-registering would cause subtle dispatch ambiguities.
        """
        if any(m.name == mode.name for m in self.modes):
            raise ValueError(f"AgentMode {mode.name!r} already registered")
        transaction = Scope(tag=f"mode:{mode.name}")
        self.modes.append(mode)

        def _remove_mode() -> None:
            for index, registered in enumerate(self.modes):
                if registered is mode:
                    del self.modes[index]
                    break

        transaction.add(
            RegistrationHandle(_remove_mode, tag=f"mode:{mode.name}"),
        )
        previous_scope = self._active_registration_scope
        self._active_registration_scope = transaction
        try:
            mode.setup(workspace)
        except Exception:
            try:
                transaction.close()
            except Exception:  # noqa: BLE001
                pass
            raise
        finally:
            self._active_registration_scope = previous_scope

        return RegistrationHandle(
            transaction.close,
            tag=f"mode:{mode.name}",
        )

    def active_mode_names(self, ctx: "HookContext") -> set[str]:
        """Return the names of every mode reporting ``is_active(ctx)``.

        Used by ``ToolRegistry.filter`` (and any other code that needs
        the runtime-active set) so per-workspace mode state never leaks
        into cross-workspace containers.
        """
        return {m.name for m in self.modes if m.is_active(ctx)}


__all__ = ["WorkspacePlugins"]
