#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
python3 "$repository_root/scripts/quality/check.py" "$@"
python3 -m unittest "$repository_root/scripts/release/test_check_v1_cutover.py"
workspace_version=$(python3 -c 'import pathlib, re; text=pathlib.Path("Cargo.toml").read_text(); print(re.search(r"(?ms)^\[workspace\.package\].*?^version\s*=\s*[\"'"'"']([^\"'"'"']+)", text).group(1))')
if [ "$workspace_version" = "1.0.0" ]; then
  "$repository_root/scripts/release/check_v1_cutover.sh" --phase preflight
  python3 "$repository_root/packaging/check_v1_plan.py"
  if [ "${PINAKOTHEKE_REHEARSAL_CHILD:-0}" != 1 ]; then
    python3 "$repository_root/scripts/release/check_v1_rehearsal.py"
  fi
else
  echo "archived v1 cutover execution skipped for post-1.0 workspace $workspace_version"
fi
