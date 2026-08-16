# -*- coding: utf-8 -*-
"""Allow running Potato via ``python -m potato``."""
from .cli.main import cli

if __name__ == "__main__":
    cli()  # pylint: disable=no-value-for-parameter
