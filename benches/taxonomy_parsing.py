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

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TAXONOMY_DIR = os.path.join(REPO_ROOT, "test_data", "taxonomies")

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


def make_controller():
    from arelle import Cntlr

    cntlr = Cntlr.Cntlr(logFileName="logToStdErr")
    cntlr.startLogging(logFileName=None)
    return cntlr


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


def run_benchmark(label: str, schema_paths: list[str], iterations: int) -> None:
    print(f"\ntaxonomy_discovery/{label}")
    print(f"  {'sample':>8}   {'time':>12}")
    samples: list[float] = []

    # Warm-up run (not recorded) to populate the OS file cache — mirrors
    # Criterion's warm_up_time before actual measurement begins.
    load_taxonomy_set(make_controller(), schema_paths)

    for i in range(iterations):
        # Fresh controller each iteration so Arelle's internal model cache is
        # cleared — mirrors Rust dropping TaxonomySet at the end of each iter.
        elapsed = load_taxonomy_set(make_controller(), schema_paths)
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

    print("Arelle XBRL taxonomy benchmark")
    print(f"Taxonomy directory : {TAXONOMY_DIR}")
    print(f"Iterations         : {ITERATIONS}")

    run_benchmark("full_dts_2020", SCHEMA_REFS_2020, ITERATIONS)
    run_benchmark("single_dts_2020", SCHEMA_REFS_SINGLE_2020, ITERATIONS)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
