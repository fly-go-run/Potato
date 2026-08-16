# -*- coding: utf-8 -*-
"""Build and runtime support for shared Office skill assets.

The six built-in Office skills have different instructions but require the
same ``scripts/office`` runtime.  Source keeps one canonical copy; package
builds materialize a real copy into every skill so installed skills and the
workspaces copied from them remain self-contained on every platform.
"""

from __future__ import annotations

import hashlib
import shutil
from pathlib import Path

OFFICE_SKILL_DIR_NAMES = (
    "docx-en",
    "docx-zh",
    "xlsx-en",
    "xlsx-zh",
    "pptx-en",
    "pptx-zh",
)
_SHARED_ASSETS_DIR_NAME = "_office_assets"
_IGNORED_ARTIFACT_NAMES = frozenset(
    {"__pycache__", "__MACOSX", ".DS_Store", "Thumbs.db", "desktop.ini"},
)


def get_shared_office_assets_dir(skills_dir: Path) -> Path:
    """Return the canonical Office runtime directory beneath *skills_dir*."""
    return skills_dir / _SHARED_ASSETS_DIR_NAME / "office"


def _office_dir(skill_dir: Path) -> Path:
    return skill_dir / "scripts" / "office"


def _ignore_artifacts(_dir: str, names: list[str]) -> set[str]:
    return {name for name in names if name in _IGNORED_ARTIFACT_NAMES}


def _file_manifest(root: Path) -> dict[str, str]:
    """Return a stable content manifest, excluding generated OS artifacts."""
    if not root.is_dir():
        raise ValueError(f"Office asset directory is missing: {root}")
    manifest: dict[str, str] = {}
    for path in sorted(root.rglob("*")):
        relative_parts = path.relative_to(root).parts
        if not path.is_file() or any(
            part in _IGNORED_ARTIFACT_NAMES for part in relative_parts
        ):
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        manifest[str(path.relative_to(root))] = digest
    if not manifest:
        raise ValueError(f"Office asset directory contains no files: {root}")
    return manifest


def validate_materialized_office_skill_assets(
    skills_dir: Path,
    *,
    source_skills_dir: Path | None = None,
) -> None:
    """Raise when a materialized Office skill is incomplete or has drifted."""
    source_root = source_skills_dir or skills_dir
    expected = _file_manifest(get_shared_office_assets_dir(source_root))
    failures: list[str] = []
    for skill_name in OFFICE_SKILL_DIR_NAMES:
        actual_dir = _office_dir(skills_dir / skill_name)
        try:
            actual = _file_manifest(actual_dir)
        except ValueError as exc:
            failures.append(str(exc))
            continue
        if actual != expected:
            missing = sorted(set(expected) - set(actual))
            unexpected = sorted(set(actual) - set(expected))
            changed = sorted(
                path
                for path in set(expected) & set(actual)
                if expected[path] != actual[path]
            )
            details: list[str] = []
            if missing:
                details.append(f"missing={missing[:3]}")
            if unexpected:
                details.append(f"unexpected={unexpected[:3]}")
            if changed:
                details.append(f"changed={changed[:3]}")
            failures.append(f"{skill_name}: " + ", ".join(details))
    if failures:
        raise ValueError(
            "Materialized Office skill assets are incomplete or drifted: "
            + "; ".join(failures),
        )


def validate_office_skill_source_layout(skills_dir: Path) -> None:
    """Raise when source retains generated per-skill Office trees."""
    _file_manifest(get_shared_office_assets_dir(skills_dir))
    stale = [
        str(_office_dir(skills_dir / name))
        for name in OFFICE_SKILL_DIR_NAMES
        if _office_dir(skills_dir / name).exists()
    ]
    if stale:
        raise ValueError(
            "Office source must keep only the shared asset tree; "
            f"found generated copies: {', '.join(stale)}",
        )


def sync_office_skill_assets(
    target_skills_dir: Path,
    *,
    source_skills_dir: Path,
) -> None:
    """Materialize canonical Office assets into all six skill directories."""
    source_assets = get_shared_office_assets_dir(source_skills_dir)
    _file_manifest(source_assets)
    for skill_name in OFFICE_SKILL_DIR_NAMES:
        skill_dir = target_skills_dir / skill_name
        if not (skill_dir / "SKILL.md").is_file():
            message = "Office skill directory is missing SKILL.md"
            raise ValueError(f"{message}: {skill_dir}")
        target_assets = _office_dir(skill_dir)
        if target_assets.exists():
            shutil.rmtree(target_assets)
        target_assets.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(source_assets, target_assets, ignore=_ignore_artifacts)
    validate_materialized_office_skill_assets(
        target_skills_dir,
        source_skills_dir=source_skills_dir,
    )


def materialize_office_assets_for_builtin_copy(
    source_skill_dir: Path,
    target_skill_dir: Path,
) -> None:
    """Fill generated assets when an editable builtin is copied to a workspace.

    Wheels already contain materialized assets.  Editable/source installs do
    not, so this runs only for a source skill directly under the packaged
    ``agents/skills`` root and never changes custom skills with similar names.
    """
    skills_dir = Path(__file__).resolve().parent / "skills"
    try:
        source_parent = source_skill_dir.resolve().parent
        is_packaged_source = source_parent == skills_dir.resolve()
    except OSError:
        is_packaged_source = False
    if (
        not is_packaged_source
        or source_skill_dir.name not in OFFICE_SKILL_DIR_NAMES
        or _office_dir(target_skill_dir).is_dir()
    ):
        return
    source_assets = get_shared_office_assets_dir(skills_dir)
    if not source_assets.is_dir():
        raise RuntimeError(
            "Built-in Office assets are missing from this installation; "
            "reinstall Potato from a complete package.",
        )
    target_assets = _office_dir(target_skill_dir)
    target_assets.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source_assets, target_assets, ignore=_ignore_artifacts)
    if _file_manifest(target_assets) != _file_manifest(source_assets):
        message = "Failed to materialize complete Office assets"
        raise RuntimeError(f"{message} for {target_skill_dir}")
