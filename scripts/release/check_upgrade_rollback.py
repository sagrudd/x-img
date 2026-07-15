#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Exercise package lifecycle and metadata rollback without local media bytes."""

from __future__ import annotations

import hashlib
import argparse
import json
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
DEBIAN_IMAGE = "debian@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818"
FEDORA_IMAGE = "fedora@sha256:99e203b80b1c3d8f7e161ec10a68fd02b081ef83a3963553e513c82846b97814"


def run(*command: str) -> None:
    subprocess.run(command, cwd=ROOT, check=True)


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def docker_lifecycle(
    image: str,
    docker_platform: str,
    baseline: pathlib.Path,
    candidate: pathlib.Path,
    baseline_version: str,
    candidate_version: str,
    package_kind: str,
) -> None:
    with tempfile.TemporaryDirectory(prefix="x-img-release-state-") as temporary:
        state = pathlib.Path(temporary)
        sentinel = state / "metadata-snapshot.json"
        sentinel.write_text(
            json.dumps(
                {
                    "schema_version": "x-img.release-lifecycle-fixture.v1",
                    "endpoint_id": "endpoint-synthetic-01",
                    "objectstore_id": "store-synthetic-01",
                    "object_checksum": "a" * 64,
                    "review_state": "New",
                },
                sort_keys=True,
            )
            + "\n"
        )
        before = digest(sentinel)
        if package_kind == "deb":
            install_baseline = f"dpkg -i /baseline/{baseline.name}"
            install_candidate = f"dpkg -i /candidate/{candidate.name}"
            rollback = f"dpkg -i --force-downgrade /baseline/{baseline.name}"
            remove = "dpkg -r x-img"
        else:
            install_baseline = f"rpm -Uvh /baseline/{baseline.name}"
            install_candidate = f"rpm -Uvh /candidate/{candidate.name}"
            rollback = f"rpm -Uvh --oldpackage /baseline/{baseline.name}"
            remove = "rpm -e x-img"
        verify_baseline = (
            f"test \"$(x-img --version)\" = \"x-img {baseline_version}\"; "
            f"grep -q '\"product_version\": \"{baseline_version}\"' "
            "/usr/share/x-img/monas/product-bootstrap.json"
        )
        verify_candidate = (
            f"test \"$(x-img --version)\" = \"x-img {candidate_version}\"; "
            f"grep -q '\"product_version\": \"{candidate_version}\"' "
            "/usr/share/x-img/monas/product-bootstrap.json"
        )
        script = "; ".join(
            [
                "set -eu",
                install_baseline,
                verify_baseline,
                install_candidate,
                verify_candidate,
                rollback,
                verify_baseline,
                remove,
                "test -f /var/lib/x-img/metadata-snapshot.json",
            ]
        )
        run(
            "docker",
            "run",
            "--rm",
            "--platform",
            docker_platform,
            "--network=none",
            "--tmpfs",
            "/tmp",
            "-v",
            f"{baseline.parent}:/baseline:ro",
            "-v",
            f"{candidate.parent}:/candidate:ro",
            "-v",
            f"{state}:/var/lib/x-img",
            image,
            "sh",
            "-c",
            script,
        )
        if digest(sentinel) != before:
            raise SystemExit(f"package lifecycle changed x-img metadata: {candidate.name}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-dist", type=pathlib.Path, required=True)
    parser.add_argument("--baseline-version", required=True)
    args = parser.parse_args()
    version = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=ROOT
        )
    )["packages"][0]["version"]
    run("python3", "packaging/check.py", "--dist", str(ROOT / "dist"), "--version", version)

    matrix = (
        ("x86_64", "amd64", "x86_64", "linux/amd64"),
        ("arm64", "arm64", "aarch64", "linux/arm64"),
    )
    for directory, deb_arch, rpm_arch, docker_platform in matrix:
        baseline_deb = args.baseline_dist / "linux" / directory / f"x-img-{args.baseline_version}-linux-{deb_arch}.deb"
        candidate_deb = ROOT / "dist/linux" / directory / f"x-img-{version}-linux-{deb_arch}.deb"
        baseline_rpm = args.baseline_dist / "linux" / directory / f"x-img-{args.baseline_version}-linux-{rpm_arch}.rpm"
        candidate_rpm = ROOT / "dist/linux" / directory / f"x-img-{version}-linux-{rpm_arch}.rpm"
        for package in (baseline_deb, candidate_deb, baseline_rpm, candidate_rpm):
            if not package.is_file():
                raise SystemExit(f"missing upgrade/rollback package: {package}")
        docker_lifecycle(
            DEBIAN_IMAGE, docker_platform, baseline_deb, candidate_deb,
            args.baseline_version, version, "deb"
        )
        docker_lifecycle(
            FEDORA_IMAGE, docker_platform, baseline_rpm, candidate_rpm,
            args.baseline_version, version, "rpm"
        )

    run("cargo", "+1.97.0", "test", "-p", "x-img-core", "migration_backup")
    run(
        "scripts/contracts/check.sh",
        "--sibling-root",
        str(ROOT.parent),
        "--sibling",
        "monas",
        "--sibling",
        "DASObjectStore",
    )
    print("upgrade/rollback acceptance passed: packages, metadata, Monas, DASObjectStore")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
