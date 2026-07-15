#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Generate a deterministic CycloneDX inventory for an x-img release."""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--dist", type=pathlib.Path, required=True)
    args = parser.parse_args()
    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--locked"], cwd=ROOT
        )
    )
    workspace = set(metadata["workspace_members"])
    components = []
    for package in sorted(metadata["packages"], key=lambda item: (item["name"], item["version"])):
        if package["id"] in workspace:
            continue
        component = {
            "type": "library",
            "bom-ref": f"pkg:cargo/{package['name']}@{package['version']}",
            "name": package["name"],
            "version": package["version"],
            "purl": f"pkg:cargo/{package['name']}@{package['version']}",
        }
        if package.get("license"):
            component["licenses"] = [{"expression": package["license"]}]
        components.append(component)
    components.append(
        {
            "type": "application",
            "bom-ref": f"pkg:generic/x-img-firefox@{args.version}",
            "name": "x-img-firefox",
            "version": args.version,
            "licenses": [{"expression": "MPL-2.0"}],
        }
    )
    document = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.6",
        "serialNumber": f"urn:uuid:00000000-0000-4000-8000-{args.version.replace('.', '').ljust(12, '0')}",
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "bom-ref": f"pkg:github/sagrudd/x-img@{args.version}",
                "name": "x-img",
                "version": args.version,
                "purl": f"pkg:github/sagrudd/x-img@{args.version}",
                "licenses": [{"expression": "MPL-2.0"}],
            }
        },
        "components": components,
    }
    args.dist.mkdir(parents=True, exist_ok=True)
    destination = args.dist / f"x-img-{args.version}.cdx.json"
    destination.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    print(destination)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
