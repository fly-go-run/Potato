#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Validate or materialize the generated Office runtime in built-in skills.

Examples:
  python scripts/sync_office_skill_assets.py --check-source
  python scripts/sync_office_skill_assets.py --sync /tmp/build/potato/agents/skills
  python scripts/sync_office_skill_assets.py --check-materialized /tmp/build/potato/agents/skills
"""

from __future__ import annotations

import argparse
import runpy
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_SKILLS_DIR = REPO_ROOT / "src" / "potato" / "agents" / "skills"
_HELPERS = runpy.run_path(
    str(REPO_ROOT / "src" / "potato" / "agents" / "office_skill_assets.py"),
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument(
        "--check-source",
        action="store_true",
        help="verify source keeps one canonical Office asset tree",
    )
    group.add_argument(
        "--sync",
        type=Path,
        metavar="SKILLS_DIR",
        help="materialize self-contained Office assets under SKILLS_DIR",
    )
    group.add_argument(
        "--check-materialized",
        type=Path,
        metavar="SKILLS_DIR",
        help="verify all six generated skill copies match the canonical assets",
    )
    args = parser.parse_args()

    if args.check_source:
        _HELPERS["validate_office_skill_source_layout"](SOURCE_SKILLS_DIR)
        print(f"Office source layout is deduplicated: {SOURCE_SKILLS_DIR}")
        return
    if args.sync is not None:
        _HELPERS["sync_office_skill_assets"](
            args.sync,
            source_skills_dir=SOURCE_SKILLS_DIR,
        )
        print(f"Office assets materialized: {args.sync}")
        return
    _HELPERS["validate_materialized_office_skill_assets"](
        args.check_materialized,
        source_skills_dir=SOURCE_SKILLS_DIR,
    )
    print(f"Office assets are complete and in sync: {args.check_materialized}")


if __name__ == "__main__":
    main()
