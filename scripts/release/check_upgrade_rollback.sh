#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu
baseline_dist=${BASELINE_DIST:?set BASELINE_DIST to an extracted prior release artifact root}
baseline_version=${BASELINE_VERSION:?set BASELINE_VERSION to the prior SemVer}
exec python3 "$(dirname "$0")/check_upgrade_rollback.py" \
  --baseline-dist "$baseline_dist" --baseline-version "$baseline_version"
