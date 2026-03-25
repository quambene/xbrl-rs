#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "arelle-release~=2.38.13",
# ]
# ///
"""
Memory benchmark for XBRL parsing using Arelle.

Measures peak memory for taxonomy discovery + instance validation.

Usage:
    # Peak RSS: /usr/bin/time -v uv run benches/bench_memory.py
"""

import os
import sys

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TAXONOMY_DIR = os.path.join(REPO_ROOT, "test_data", "taxonomies")
INSTANCE_PATH = os.path.join(
    REPO_ROOT, "test_data", "instances", "balance_sheet_v64.xml"
)

# Single taxonomy entry point.
SCHEMA_REF = os.path.join(
    TAXONOMY_DIR, "de-bra-2020-04-01", "de-bra-2020-04-01-shell-fiscal.xsd"
)


def main() -> int:
    try:
        from arelle import Cntlr  # noqa: F401
    except ImportError:
        print(
            "ERROR: arelle is not installed. Run: uv run benches/bench_memory.py",
            file=sys.stderr,
        )
        return 1

    # Load single taxonomy entry point
    from arelle.ModelFormulaObject import FormulaOptions

    controller = Cntlr.Cntlr(logFileName="logToStdErr")
    controller.startLogging(logFileName=None)
    controller.modelManager.formulaOptions = FormulaOptions()

    url = "file://" + os.path.abspath(SCHEMA_REF).replace("\\", "/")
    model_xbrl = controller.modelManager.load(url)

    # Load and validate instance document
    instance_url = (
        "file://" + os.path.abspath(INSTANCE_PATH).replace("\\", "/")
    )
    instance_model = controller.modelManager.load(instance_url)

    from arelle.ValidateXbrl import ValidateXbrl

    validator = ValidateXbrl(instance_model)
    validator.validate(instance_model)
    validator.close()

    return 0


if __name__ == "__main__":
    sys.exit(main())
