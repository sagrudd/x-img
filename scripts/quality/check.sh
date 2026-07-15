#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
python3 "$repository_root/scripts/quality/check.py" "$@"
python3 -m unittest "$repository_root/scripts/release/test_check_v1_cutover.py"
"$repository_root/scripts/release/check_v1_cutover.sh" --phase preflight
python3 "$repository_root/packaging/check_v1_plan.py"
if [ "${PINAKOTHEKE_REHEARSAL_CHILD:-0}" != 1 ]; then
  python3 "$repository_root/scripts/release/check_v1_rehearsal.py"
fi
