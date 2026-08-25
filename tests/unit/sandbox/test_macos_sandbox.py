# -*- coding: utf-8 -*-
"""Seatbelt profile compilation for nested write denials."""
from __future__ import annotations

from potato.sandbox.config import MountSpec, SandboxConfig, SandboxMode
from potato.sandbox.macos_sandbox import MacOSSandbox


def test_nested_hooks_deny_uses_require_not() -> None:
    config = SandboxConfig(
        mode=SandboxMode.SEATBELT,
        workspace_dir="/proj",
        mounts=[MountSpec(path="/proj", writable=True)],
        deny_paths=["/proj/.git/hooks", "/Users/me/.ssh"],
        network_allow=[],
    )
    profile = MacOSSandbox(config)._compile_seatbelt_profile()
    assert "(require-all" in profile
    assert '(require-not (subpath "/proj/.git/hooks"))' in profile
    assert '(deny file-write*\n  (subpath "/Users/me/.ssh"))' in profile
    # Hooks stay readable so `git commit` can see them.
    assert '(deny file-read*\n  (subpath "/proj/.git/hooks"))' not in profile
