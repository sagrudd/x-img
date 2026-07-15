#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu
cd "$(dirname "$0")/../.."
exec python3 scripts/audit/check.py
