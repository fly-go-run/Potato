# -*- coding: utf-8 -*-
"""Shared write-capability boundary for file tools and the shell sandbox.

Default writable roots match Codex / official DeepSeek Harness
``workspace-write``: the workspace, the coding-project dir, ``/tmp``, and
the process temp dir (``$TMPDIR`` / ``tempfile.gettempdir()``).

Hard-denied locations cannot be opened by an ALLOW rule:

* ``.git/hooks`` under every project root
* the on-disk governance policy directory
* ``agent.json`` in those project roots (persisted agent instructions)

Writes outside the default roots are not a hard deny: they become ASK
unless a user rule already granted that path.
"""
from __future__ import annotations

import os
import tempfile
from pathlib import Path
from typing import Iterable

from .policy import (
    DEFAULT_SANDBOX_DENY_PATHS,
    FILE_READ_TOOLS,
    FILE_WRITE_TOOLS,
    GovernanceAction,
    GovernanceDecision,
    ToolCallSpec,
)

DENIED_PROJECT_SUBPATHS: tuple[str, ...] = (os.path.join(".git", "hooks"),)
_AGENT_INSTRUCTION_FILES: tuple[str, ...] = ("agent.json",)
_FILE_SEARCH_TOOLS: frozenset[str] = frozenset({"Glob", "Grep", "AstSearch"})


def canonical_path(path: str | Path) -> Path:
    """Resolve *path* the way containment checks must see it.

    Existing prefixes are ``realpath``'d so symlink / ``..`` escapes cannot
    walk out of a granted root. Missing suffix components are re-attached
    after the deepest existing ancestor is resolved.
    """
    raw = Path(path).expanduser()
    if not raw.is_absolute():
        raw = Path.cwd() / raw

    current = raw
    missing: list[str] = []
    while True:
        try:
            resolved = current.resolve(strict=True)
            break
        except (FileNotFoundError, OSError):
            if current.parent == current:
                resolved = current
                break
            missing.append(current.name)
            current = current.parent

    for name in reversed(missing):
        if name == "." or name == "":
            continue
        if name == "..":
            resolved = resolved.parent
            continue
        resolved = resolved / name
    return resolved


def default_writable_roots(
    workspace_dir: str | Path,
    coding_project_dir: str | Path | None = None,
) -> list[Path]:
    """Return the default Auto / workspace-write roots (canonical)."""
    roots: list[Path] = [canonical_path(workspace_dir)]
    if coding_project_dir:
        coding = canonical_path(coding_project_dir)
        if coding not in roots:
            roots.append(coding)

    for extra in _platform_temp_roots():
        if extra not in roots:
            roots.append(extra)
    return roots


def _platform_temp_roots() -> list[Path]:
    seen: set[Path] = set()
    roots: list[Path] = []
    candidates = ["/tmp", os.environ.get("TMPDIR", ""), tempfile.gettempdir()]
    for raw in candidates:
        if not raw:
            continue
        try:
            path = canonical_path(raw)
        except (OSError, RuntimeError):
            continue
        if path in seen:
            continue
        seen.add(path)
        roots.append(path)
    return roots


def read_only_subpaths(project_roots: Iterable[Path]) -> list[Path]:
    """Project-relative locations that stay read-only in a writable root."""
    denied: list[Path] = []
    for root in project_roots:
        for rel in DENIED_PROJECT_SUBPATHS:
            denied.append(canonical_path(root / rel))
        for name in _AGENT_INSTRUCTION_FILES:
            denied.append(canonical_path(root / name))
    return denied


def extra_denied_roots(
    *,
    policy_dir: str | Path | None = None,
    credential_paths: Iterable[str] | None = None,
) -> list[Path]:
    """Absolute locations that file writes must never open."""
    denied: list[Path] = []
    if policy_dir:
        denied.append(canonical_path(policy_dir))
    try:
        from .global_rules import global_rules_path

        denied.append(canonical_path(global_rules_path().parent))
    except Exception:
        pass
    for raw in credential_paths if credential_paths is not None else (
        DEFAULT_SANDBOX_DENY_PATHS
    ):
        try:
            denied.append(canonical_path(Path(raw).expanduser()))
        except (OSError, RuntimeError):
            continue
    return denied


def is_under(path: Path, root: Path) -> bool:
    """True when *path* is *root* or a descendant (both already canonical)."""
    if path == root:
        return True
    return root in path.parents


def classify_write_target(
    target: str,
    writable_roots: Iterable[Path],
    denied_paths: Iterable[Path],
) -> str:
    """Classify one write target.

    Returns
    -------
    ``"denied"``
        Hard boundary (hooks, governance, credentials, agent.json).
    ``"inside"``
        Inside a default writable root and not denied.
    ``"outside"``
        Outside the default roots (Desktop, home, …).
    """
    path = canonical_path(target)
    for denied in denied_paths:
        if is_under(path, denied):
            return "denied"
    for root in writable_roots:
        if is_under(path, root):
            return "inside"
    return "outside"


def is_file_write_tool(tool_name: str) -> bool:
    """True for builtin writers and plugin file tools that are not reads."""
    name = str(tool_name or "")
    if name in FILE_WRITE_TOOLS:
        return True
    if name in FILE_READ_TOOLS or name in _FILE_SEARCH_TOOLS:
        return False
    try:
        from .tool_registry import DEFAULT_REGISTRY

        return DEFAULT_REGISTRY.get_type(name) == "file"
    except Exception:
        return False


def refine_file_write_decision(
    decision: GovernanceDecision,
    tc_spec: ToolCallSpec,
    *,
    workspace_dir: str | Path,
    coding_project_dir: str | Path | None = None,
    policy_dir: str | Path | None = None,
) -> GovernanceDecision:
    """Apply the shared write boundary on top of a policy decision.

    ALLOW rules may still skip a prompt for an explicitly granted outside
    path. They cannot punch through a hard-denied location. Fallback ALLOW
    (no rule hit) for an outside path becomes ASK.
    """
    if not is_file_write_tool(tc_spec.tool_name):
        return decision
    if decision.action is GovernanceAction.DENY:
        return decision
    if not tc_spec.target:
        return decision

    project_roots = default_writable_roots(
        workspace_dir,
        coding_project_dir,
    )
    # Temp roots are writable but do not grow project-relative deny paths.
    project_only = [canonical_path(workspace_dir)]
    if coding_project_dir:
        coding = canonical_path(coding_project_dir)
        if coding not in project_only:
            project_only.append(coding)

    denied = read_only_subpaths(project_only) + extra_denied_roots(
        policy_dir=policy_dir,
    )
    kind = classify_write_target(tc_spec.target, project_roots, denied)
    escaped = _is_symlink_escape(
        tc_spec.target,
        project_roots,
        spelled_roots=_spelled_roots(workspace_dir, coding_project_dir),
    )

    if kind == "denied":
        return GovernanceDecision(
            action=GovernanceAction.DENY,
            reason=(
                f"Write to '{tc_spec.target}' is blocked by a hard "
                "capability boundary"
            ),
            findings=decision.findings,
            source="write_boundary",
        )
    if kind == "outside" and decision.action is GovernanceAction.ALLOW:
        # Fallback ALLOW must not open Desktop/home. A workspace glob that
        # only matched the lexical path (symlink / ``..`` escape) is also
        # not an explicit grant of the real destination.
        if decision.source == "fallback" or escaped:
            return GovernanceDecision(
                action=GovernanceAction.ASK,
                reason=(
                    f"Write to '{tc_spec.target}' is outside the default "
                    "writable roots"
                ),
                findings=decision.findings,
                source="write_boundary",
            )
    return decision


def _spelled_path(path: str | Path) -> Path:
    """Absolute path without resolving symlinks."""
    raw = Path(path).expanduser()
    if not raw.is_absolute():
        raw = Path.cwd() / raw
    return raw


def _spelled_roots(
    workspace_dir: str | Path,
    coding_project_dir: str | Path | None = None,
) -> list[Path]:
    roots = [_spelled_path(workspace_dir)]
    if coding_project_dir:
        coding = _spelled_path(coding_project_dir)
        if coding not in roots:
            roots.append(coding)
    return roots


def _lexical_path(path: str | Path) -> Path:
    return Path(os.path.normpath(str(_spelled_path(path))))


def _norm_under(path: Path, root: Path) -> bool:
    try:
        Path(os.path.normpath(str(path))).relative_to(
            os.path.normpath(str(root)),
        )
        return True
    except ValueError:
        return False


def _raw_under(target: str, roots: Iterable[Path]) -> bool:
    """True when the raw (un-normalized) string sits under a spelled root."""
    raw = os.path.expanduser(str(target))
    for root in roots:
        prefix = str(root)
        if raw == prefix or raw.startswith(prefix + os.sep):
            return True
    return False


def _is_symlink_escape(
    target: str,
    writable_roots: Iterable[Path],
    spelled_roots: Iterable[Path],
) -> bool:
    """True when the spelled path looks inside a root but realpath does not."""
    lexical = _lexical_path(target)
    real = canonical_path(target)
    spelled = list(spelled_roots)
    lexical_inside = any(
        is_under(lexical, root) or _norm_under(lexical, root)
        for root in list(writable_roots) + spelled
    ) or _raw_under(target, spelled)
    real_inside = any(is_under(real, root) for root in writable_roots)
    return lexical_inside and not real_inside


def validate_extra_writable(
    path: str,
    *,
    workspace_dir: str | Path,
    coding_project_dir: str | Path | None = None,
    policy_dir: str | Path | None = None,
) -> tuple[str | None, str | None]:
    """Return ``(canonical_path, error)`` for one extra writable root."""
    raw = str(path or "").strip()
    if not raw:
        return None, "additional_writable_path is empty"
    try:
        resolved = canonical_path(raw)
    except (OSError, RuntimeError):
        return None, "additional_writable_path could not be resolved"
    project = [canonical_path(workspace_dir)]
    if coding_project_dir:
        coding = canonical_path(coding_project_dir)
        if coding not in project:
            project.append(coding)
    denied = read_only_subpaths(project) + extra_denied_roots(
        policy_dir=policy_dir,
    )
    if classify_write_target(str(resolved), project, denied) == "denied":
        return None, (
            f"additional_writable_path '{resolved}' is a hard "
            "capability boundary"
        )
    return str(resolved), None


def assert_inside_writable_roots(
    target: str,
    *,
    workspace_dir: str | Path,
    coding_project_dir: str | Path | None = None,
    policy_dir: str | Path | None = None,
) -> tuple[str | None, str | None]:
    """Return ``(canonical_path, error)`` for a side-effect write.

    Browser downloads and other tools without an ASK card must stay
    inside the default writable roots. Outside / hard-denied paths
    fail closed here instead of writing first.
    """
    raw = str(target or "").strip()
    if not raw:
        return None, "output path is empty"
    try:
        resolved = canonical_path(raw)
    except (OSError, RuntimeError):
        return None, "output path could not be resolved"
    project = [canonical_path(workspace_dir)]
    if coding_project_dir:
        coding = canonical_path(coding_project_dir)
        if coding not in project:
            project.append(coding)
    roots = default_writable_roots(workspace_dir, coding_project_dir)
    denied = read_only_subpaths(project) + extra_denied_roots(
        policy_dir=policy_dir,
    )
    kind = classify_write_target(str(resolved), roots, denied)
    if kind == "denied":
        return None, (
            f"Write to '{resolved}' is blocked by a hard "
            "capability boundary"
        )
    if kind == "outside":
        return None, (
            f"Write to '{resolved}' is outside the default "
            "writable roots"
        )
    return str(resolved), None


def sandbox_deny_paths(
    *,
    workspace_dir: str | Path,
    coding_project_dir: str | Path | None = None,
    policy_dir: str | Path | None = None,
) -> list[str]:
    """Deny-list compiled into the shell sandbox config."""
    project_only = [canonical_path(workspace_dir)]
    if coding_project_dir:
        coding = canonical_path(coding_project_dir)
        if coding not in project_only:
            project_only.append(coding)
    paths = [
        *DEFAULT_SANDBOX_DENY_PATHS,
        *[str(p) for p in read_only_subpaths(project_only)],
        *[
            str(p)
            for p in extra_denied_roots(
                policy_dir=policy_dir,
                credential_paths=(),
            )
        ],
    ]
    # Preserve order while dropping duplicates.
    seen: set[str] = set()
    unique: list[str] = []
    for item in paths:
        if item in seen:
            continue
        seen.add(item)
        unique.append(item)
    return unique
