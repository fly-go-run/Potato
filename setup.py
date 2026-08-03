# -*- coding: utf-8 -*-
"""Setuptools entry point, including generated Office skill package data."""

from pathlib import Path
import runpy
import shutil

from setuptools import setup
from setuptools.command.build_py import build_py as _build_py

_ROOT = Path(__file__).resolve().parent
_OFFICE_ASSET_HELPERS = runpy.run_path(
    str(_ROOT / "src" / "qwenpaw" / "agents" / "office_skill_assets.py"),
)
_sync_office_skill_assets = _OFFICE_ASSET_HELPERS["sync_office_skill_assets"]


class _BuildPyWithOfficeSkillAssets(_build_py):
    """Materialize self-contained Office skills in build output only."""

    def run(self) -> None:
        super().run()
        # Editable installs (pip/uv `-e .`) run straight off the source
        # tree: build_py copies no package data into build_lib, so there is
        # nothing to materialize — and the sync would fail on the empty dir.
        if getattr(self, "editable_mode", False):
            return
        build_agents_dir = Path(self.build_lib) / "qwenpaw" / "agents"
        build_skills_dir = build_agents_dir / "skills"
        if not build_skills_dir.is_dir():
            return
        _sync_office_skill_assets(
            build_skills_dir,
            source_skills_dir=_ROOT / "src" / "qwenpaw" / "agents" / "skills",
        )
        # The six generated skills are self-contained; the source-only helper
        # tree is not needed in a wheel and would add a seventh copy.
        shutil.rmtree(build_skills_dir / "_office_assets")


setup(cmdclass={"build_py": _BuildPyWithOfficeSkillAssets})
