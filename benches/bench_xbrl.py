#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "arelle-release~=2.38.13",
# ]
# ///
"""
Benchmark for XBRL taxonomy discovery using Arelle.

Measures the time to load the full DTS for the 2020-04-01 German taxonomy
suites, for comparison with the Rust xbrl-rs benchmarks.

Usage:
    uv run benches/taxonomy_parsing.py
"""

import os
import statistics
import sys
import time

from arelle.ModelFormulaObject import FormulaOptions
from arelle import Cntlr
from arelle.ValidateXbrl import ValidateXbrl

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TAXONOMY_DIR = os.path.join(REPO_ROOT, "test_data", "taxonomies")
INSTANCE_PATH = os.path.join(
    REPO_ROOT, "test_data", "instances", "balance_sheet_v64.xml"
)

# All 6 entry points as used in a real HGB instance document.
SCHEMA_REFS_2020 = [
    os.path.join(TAXONOMY_DIR, "de-gcd-2020-04-01",
                 "de-gcd-2020-04-01-shell.xsd"),
    os.path.join(TAXONOMY_DIR, "de-gaap-ci-2020-04-01",
                 "de-gaap-ci-2020-04-01-shell-fiscal.xsd"),
    os.path.join(TAXONOMY_DIR, "de-bra-2020-04-01",
                 "de-bra-2020-04-01-shell-fiscal.xsd"),
    os.path.join(
        TAXONOMY_DIR,
        "de-fi-2020-04-01",
        "de-fi-2020-04-01-shell-staffelform-fiscal.xsd",
    ),
    os.path.join(TAXONOMY_DIR, "de-ins-2020-04-01",
                 "de-ins-2020-04-01-shell-fiscal.xsd"),
    os.path.join(
        TAXONOMY_DIR,
        "de-pi-2020-04-01",
        "de-pi-2020-04-01-shell-staffelform-fiscal.xsd",
    ),
]

# Single entry point benchmark.
SCHEMA_REFS_SINGLE_2020 = [
    os.path.join(TAXONOMY_DIR, "de-bra-2020-04-01",
                 "de-bra-2020-04-01-shell-fiscal.xsd"),
]

ITERATIONS = 5


def create_controller():

    controller = Cntlr.Cntlr(logFileName="logToStdErr")
    controller.startLogging(logFileName=None)
    return controller


def prepare_bench():
    """Set up the instance model once, return a closure that only measures validation."""
    controller = create_controller()
    opts = FormulaOptions()
    # Disable formula validation as it is not supported by xbrl-rs yet.
    opts.formulaAction = "none"
    controller.modelManager.formulaOptions = opts
    model = load_instance(controller)
    return lambda: validate_instance(controller, model)


def load_taxonomy_set(cntlr, schema_paths: list[str]) -> float:
    """Load all schemas sequentially, returning total elapsed seconds.

    Arelle builds a fully independent model per load() call with no cross-call
    deduplication, so each schema is timed and closed individually. This differs
    from TaxonomySet::discover(), which merges all entry points into one DTS and
    parses each file exactly once — making the Rust benchmark inherently faster for
    entry point sets that share schemas (de-bra and de-ins both import de-gaap-ci).
    """
    total = 0.0

    for path in schema_paths:
        url = "file://" + os.path.abspath(path).replace("\\", "/")
        t0 = time.perf_counter()
        model_xbrl = cntlr.modelManager.load(url)
        total += time.perf_counter() - t0
        if model_xbrl is not None:
            cntlr.modelManager.close(model_xbrl)

    return total


def load_instance(cntlr) -> "ModelXbrl":
    """Load the instance document, returning the model."""
    url = "file://" + os.path.abspath(INSTANCE_PATH).replace("\\", "/")
    return cntlr.modelManager.load(url)


def validate_instance(cntlr, model_xbrl) -> float:
    """Validate a loaded instance, returning elapsed seconds."""
    t0 = time.perf_counter()
    validator = ValidateXbrl(model_xbrl)
    validator.validate(model_xbrl)
    elapsed = time.perf_counter() - t0
    validator.close()
    return elapsed


def run_benchmark(label: str, bench_fn, iterations: int) -> None:
    print(f"\n{label}")
    print(f"  {'sample':>8}   {'time':>12}")
    samples: list[float] = []

    # Warm-up run (not recorded) to populate the OS file cache — mirrors
    # Criterion's warm_up_time before actual measurement begins.
    bench_fn()

    for i in range(iterations):
        elapsed = bench_fn()
        samples.append(elapsed)
        print(f"  {i + 1:>8}   {elapsed * 1000:>10.2f} ms")

    mean = statistics.mean(samples)
    stdev = statistics.stdev(samples) if len(samples) > 1 else 0.0
    print(f"  {'mean':>8}   {mean * 1000:>10.2f} ms  ± {stdev * 1000:.2f} ms")
    print(f"  {'min':>8}   {min(samples) * 1000:>10.2f} ms")
    print(f"  {'max':>8}   {max(samples) * 1000:>10.2f} ms")


def main() -> int:
    try:
        from arelle import Cntlr  # noqa: F401
    except ImportError:
        print(
            "ERROR: arelle is not installed. Run: uv run benches/taxonomy_parsing.py",
            file=sys.stderr,
        )
        return 1

    print("Arelle XBRL benchmark")
    print(f"Taxonomy directory : {TAXONOMY_DIR}")
    print(f"Instance document  : {INSTANCE_PATH}")
    print(f"Iterations         : {ITERATIONS}")

    run_benchmark(
        "taxonomy_discovery/full_dts_2020",
        lambda: load_taxonomy_set(create_controller(), SCHEMA_REFS_2020),
        ITERATIONS,
    )
    run_benchmark(
        "taxonomy_discovery/single_dts_2020",
        lambda: load_taxonomy_set(
            create_controller(), SCHEMA_REFS_SINGLE_2020),
        ITERATIONS,
    )

    run_benchmark("validate_instance", prepare_bench(), ITERATIONS)

    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
