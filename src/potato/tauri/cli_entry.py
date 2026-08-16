# -*- coding: utf-8 -*-
"""PyInstaller entry point for the bundled Potato CLI."""
from __future__ import annotations

import multiprocessing as mp

from potato.cli.main import cli


if __name__ == "__main__":
    mp.freeze_support()
    cli()  # pylint: disable=no-value-for-parameter
