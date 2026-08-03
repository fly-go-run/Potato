# -*- coding: utf-8 -*-
import os
from pathlib import Path
import subprocess
import sys

import pytest

from qwenpaw.agents.office_skill_assets import (
    OFFICE_SKILL_DIR_NAMES,
    get_shared_office_assets_dir,
    sync_office_skill_assets,
    validate_materialized_office_skill_assets,
    validate_office_skill_source_layout,
)
from qwenpaw.agents.skill_system.registry import get_builtin_skills_dir
from qwenpaw.agents.skill_system.store import copy_skill_dir

REPO_ROOT = Path(__file__).resolve().parents[3]


def _create_skill_roots(skills_dir: Path) -> None:
    for skill_name in OFFICE_SKILL_DIR_NAMES:
        skill_dir = skills_dir / skill_name
        skill_dir.mkdir(parents=True)
        (skill_dir / "SKILL.md").write_text(
            "---\nname: office\n---\n",
            encoding="utf-8",
        )


def test_office_source_keeps_one_canonical_asset_tree() -> None:
    skills_dir = get_builtin_skills_dir()

    validate_office_skill_source_layout(skills_dir)
    assert (get_shared_office_assets_dir(skills_dir) / "validate.py").is_file()


def test_sync_materializes_complete_matching_assets_for_every_office_skill(
    tmp_path: Path,
) -> None:
    source_skills = get_builtin_skills_dir()
    built_skills = tmp_path / "qwenpaw" / "agents" / "skills"
    _create_skill_roots(built_skills)

    sync_office_skill_assets(built_skills, source_skills_dir=source_skills)
    validate_materialized_office_skill_assets(
        built_skills,
        source_skills_dir=source_skills,
    )

    for skill_name in OFFICE_SKILL_DIR_NAMES:
        office_dir = built_skills / skill_name / "scripts" / "office"
        assert (office_dir / "validate.py").is_file()
        assert (office_dir / "validators" / "base.py").is_file()
        wml_schema = office_dir / "schemas" / "ISO-IEC29500-4_2016" / "wml.xsd"
        assert wml_schema.is_file()

    drifted = built_skills / "docx-en" / "scripts" / "office" / "validate.py"
    drifted.write_text("changed", encoding="utf-8")
    with pytest.raises(ValueError, match="drifted"):
        validate_materialized_office_skill_assets(
            built_skills,
            source_skills_dir=source_skills,
        )


def test_copying_an_editable_builtin_materializes_its_runtime_assets(
    tmp_path: Path,
) -> None:
    source = get_builtin_skills_dir() / "docx-en"
    target = tmp_path / "pool" / "docx"

    copy_skill_dir(source, target)

    assert (target / "SKILL.md").is_file()
    assert (target / "scripts" / "office" / "pack.py").is_file()
    mce_schema = target / "scripts" / "office" / "schemas" / "mce" / "mc.xsd"
    assert mce_schema.is_file()


def test_build_output_keeps_only_six_self_contained_office_skills(
    tmp_path: Path,
) -> None:
    build_base = tmp_path / "build"
    env = os.environ | {"PYTHONPYCACHEPREFIX": str(tmp_path / "pycache")}
    result = subprocess.run(
        [
            sys.executable,
            "setup.py",
            "build",
            "--build-base",
            str(build_base),
        ],
        cwd=REPO_ROOT,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stdout + result.stderr

    built_skills = build_base / "lib" / "qwenpaw" / "agents" / "skills"
    assert not (built_skills / "_office_assets").exists()
    validate_materialized_office_skill_assets(
        built_skills,
        source_skills_dir=get_builtin_skills_dir(),
    )
