#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Run the deterministic x-img fault-injection and recovery acceptance suite."""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
MATRIX = ROOT / "docs/fixtures/fault-recovery-matrix.json"

CASES: dict[str, list[str]] = {
    "ingest-backpressure-checksum": ["cargo", "+1.97.0", "test", "-p", "x-img-core", "object_ingest::tests::rejects_length_checksum_and_backpressure_without_local_buffering", "--", "--exact"],
    "authority-crash-boundaries": ["cargo", "+1.97.0", "test", "-p", "x-img-core", "reconciliation::tests::crash_boundaries_converge_without_creating_duplicate_commits", "--", "--exact"],
    "destination-authority-change": ["cargo", "+1.97.0", "test", "-p", "x-img-core", "destination_selection::tests::revalidation_never_switches_destinations", "--", "--exact"],
    "scheduler-cancel-budget": ["cargo", "+1.97.0", "test", "-p", "x-img-core", "scheduler::tests::budgets_and_cancellation_are_bounded_and_release_leases", "--", "--exact"],
    "normalizer-worker-crash": ["cargo", "+1.97.0", "test", "-p", "x-img-core", "video_normalization::tests::ledger_requires_reconciliation_after_a_crash_and_prevents_conflicting_replay", "--", "--exact"],
    "normalizer-scratch-failure": ["cargo", "+1.97.0", "test", "-p", "x-img-core", "video_normalization::tests::failure_removes_the_entire_ephemeral_scratch_directory", "--", "--exact"],
    "cache-authority-loss": ["cargo", "+1.97.0", "test", "-p", "x-img-core", "cache_alias::tests::bounds_memory_invalidates_and_surfaces_authority_unavailability", "--", "--exact"],
    "capture-policy-unavailable": ["cargo", "+1.97.0", "test", "-p", "x-img-api", "tests::default_router_fails_open_for_unconfigured_capture_policy", "--", "--exact"],
    "firefox-substitution-failure": [sys.executable, "scripts/firefox/check_image_substitution.py"],
}


def main() -> int:
    document = json.loads(MATRIX.read_text())
    assert document == json.loads(json.dumps(document)), "matrix must be JSON-safe"
    assert document.get("schema") == "x-img.fault-recovery-matrix"
    assert document.get("schema_major") == 1
    assert document.get("fixture_kind") == "synthetic"
    cases = document.get("cases")
    assert isinstance(cases, list) and cases
    ids = [case.get("id") for case in cases]
    assert len(ids) == len(set(ids)) and set(ids) == set(CASES)
    for case in cases:
        assert set(case) == {"id", "fault", "invariant"}
        assert all(isinstance(case[field], str) and case[field] for field in case)
        subprocess.run(CASES[case["id"]], cwd=ROOT, check=True)
        print(f"fault case passed: {case['id']} — {case['invariant']}")
    print(f"fault/recovery suite passed: {len(cases)} synthetic cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
