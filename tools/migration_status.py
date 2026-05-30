#!/usr/bin/env python3
"""Report combined ZMK-to-RMK porting and hardware validation status."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))

import firmware_artifact_manifest
import firmware_artifact_specs
import hardware_validation
import porting_coverage

SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


@dataclass
class SoftwareStatus:
    passed: int
    total: int
    rate: float | None
    zmk_source: dict[str, Any]
    zmk_source_clean: bool
    zmk_source_clean_errors: list[str]
    by_kind: dict[str, dict[str, int | float | None]]
    implementation: dict[str, Any]
    complete: bool
    failed: list[dict[str, str | int]]


@dataclass
class MigrationStatus:
    software: SoftwareStatus
    hardware: dict[str, Any]
    firmware_artifacts: dict[str, Any] | None
    ready_for_release_without_hardware: bool
    fully_validated: bool


def percent(rate: float | None) -> str:
    return "n/a" if rate is None else f"{rate * 100.0:.2f}%"


def software_status(
    manifest_path: Path,
    keyboard_path: Path,
    zmk_keymap_path: Path | None,
    require_zmk_source: bool,
    require_zmk_clean_source: bool,
    require_zmk_source_commit: bool,
    coverage_baseline_path: Path | None,
) -> SoftwareStatus:
    manifest = porting_coverage.load_toml(manifest_path)
    resolved_zmk_keymap = porting_coverage.resolve_zmk_keymap_path(manifest, zmk_keymap_path)
    zmk_source = porting_coverage.zmk_source_reference(resolved_zmk_keymap)
    results = porting_coverage.run(
        manifest_path,
        keyboard_path,
        resolved_zmk_keymap,
        require_zmk_source,
    )
    implementation = porting_coverage.porting_status_summary(manifest)
    passed = sum(result.passed for result in results)
    total = sum(result.total for result in results)
    by_kind = porting_coverage.coverage_by_kind(results)
    result_count, result_sha256 = porting_coverage.result_inventory_digest(results)
    baseline_failures: list[dict[str, str | int]] = []
    if coverage_baseline_path is not None:
        try:
            coverage_baseline = porting_coverage.load_toml(coverage_baseline_path)
        except OSError as e:
            failures = [f"failed to read {coverage_baseline_path}: {e}"]
        else:
            failures = porting_coverage.baseline_errors(
                coverage_baseline,
                passed,
                total,
                by_kind,
                implementation,
                result_count,
                result_sha256,
            )
        baseline_failures = [
            {
                "id": "coverage_baseline",
                "kind": "baseline",
                "passed": 0,
                "total": 1,
                "message": failure,
            }
            for failure in failures
        ]
    zmk_clean_errors = porting_coverage.zmk_source_clean_errors(zmk_source)
    zmk_source_failures = [
        {
            "id": "zmk_source_clean",
            "kind": "zmk_source",
            "passed": 0,
            "total": 1,
            "message": failure,
        }
        for failure in (
            zmk_clean_errors if require_zmk_clean_source else []
        )
    ]
    zmk_source_commit_failures = [
        {
            "id": "zmk_source_commit",
            "kind": "zmk_source",
            "passed": 0,
            "total": 1,
            "message": failure,
        }
        for failure in (
            porting_coverage.zmk_source_commit_errors(manifest, zmk_source)
            if require_zmk_source_commit
            else []
        )
    ]
    failed = [
        {
            "id": result.id,
            "kind": result.kind,
            "passed": result.passed,
            "total": result.total,
            "message": result.message,
        }
        for result in results
        if not result.ok
    ] + baseline_failures + zmk_source_failures + zmk_source_commit_failures
    by_kind_json = {
        kind: {
            "passed": bucket.passed,
            "total": bucket.total,
            "rate": bucket.rate,
        }
        for kind, bucket in by_kind.items()
    }
    complete = (
        not failed
        and implementation.total > 0
        and implementation.implemented == implementation.total
    )
    return SoftwareStatus(
        passed=passed,
        total=total,
        rate=None if total == 0 else passed / total,
        zmk_source=zmk_source,
        zmk_source_clean=not zmk_clean_errors,
        zmk_source_clean_errors=zmk_clean_errors,
        by_kind=by_kind_json,
        implementation={
            "total": implementation.total,
            "implemented": implementation.implemented,
            "rate": implementation.rate,
            "by_status": implementation.by_status,
            "remaining": implementation.remaining,
        },
        complete=complete,
        failed=failed,
    )


def hardware_status(
    manifest_path: Path,
    hardware_baseline_path: Path | None,
    evidence_paths: list[Path],
    evidence_artifact_root: Path,
    required_firmware_ref: str | None,
    required_artifact_pair_sha256: str | None,
    require_evidence_inventory: bool,
    require_evidence_artifact_paths: bool,
    extra_errors: list[str] | None = None,
) -> dict[str, Any]:
    manifest_doc = hardware_validation.load_toml(manifest_path)
    baseline_failures: list[str] = []
    if hardware_baseline_path is not None:
        try:
            hardware_baseline = hardware_validation.load_toml(hardware_baseline_path)
        except OSError as e:
            baseline_failures = [f"failed to read {hardware_baseline_path}: {e}"]
        else:
            baseline_failures = hardware_validation.hardware_baseline_errors(
                hardware_baseline,
                manifest_doc,
            )
    evidence_docs, evidence_load_errors = hardware_validation.load_evidence_docs(evidence_paths)
    manifest, evidence_errors = hardware_validation.merge_evidence(
        manifest_doc,
        evidence_docs,
        require_evidence_inventory,
    )
    summary = hardware_validation.summarize(
        manifest,
        evidence_load_errors + evidence_errors + baseline_failures + (extra_errors or []),
        Path("."),
        evidence_artifact_root,
        required_firmware_ref,
        required_artifact_pair_sha256,
        require_evidence_artifact_paths,
    )
    return hardware_validation.as_json(summary)


def firmware_artifact_status(
    artifact_manifest_path: Path | None,
    required_firmware_ref: str | None,
    artifact_root: Path,
) -> dict[str, Any] | None:
    if artifact_manifest_path is None:
        return None

    errors: list[str] = []
    resolved_artifact_root = artifact_root.resolve()
    try:
        artifact_manifest = json.loads(artifact_manifest_path.read_text(encoding="utf-8"))
    except OSError as e:
        return {
            "path": str(artifact_manifest_path),
            "firmware_ref": "",
            "artifact_count": 0,
            "pair_sha256": "",
            "errors": [f"failed to read firmware artifact manifest {artifact_manifest_path}: {e}"],
        }
    except json.JSONDecodeError as e:
        return {
            "path": str(artifact_manifest_path),
            "firmware_ref": "",
            "artifact_count": 0,
            "pair_sha256": "",
            "errors": [f"firmware artifact manifest {artifact_manifest_path} is invalid JSON: {e}"],
        }
    if not isinstance(artifact_manifest, dict):
        return {
            "path": str(artifact_manifest_path),
            "firmware_ref": "",
            "artifact_count": 0,
            "pair_sha256": "",
            "errors": [f"firmware artifact manifest {artifact_manifest_path} must be a JSON object"],
        }

    firmware_ref = str(artifact_manifest.get("firmware_ref", "")).strip()
    if not firmware_ref:
        errors.append("firmware artifact manifest missing firmware_ref")
    elif hardware_validation.is_mutable_firmware_ref(firmware_ref):
        errors.append("firmware artifact manifest firmware_ref must be immutable")
    if required_firmware_ref is not None and firmware_ref != required_firmware_ref:
        errors.append(
            "firmware artifact manifest firmware_ref "
            f"{firmware_ref!r} does not match required {required_firmware_ref!r}"
        )

    pair_sha256 = str(artifact_manifest.get("pair_sha256", "")).strip()
    if not SHA256_RE.fullmatch(pair_sha256):
        errors.append("firmware artifact manifest pair_sha256 must be a SHA256 hex string")

    artifacts = artifact_manifest.get("artifacts", [])
    if not isinstance(artifacts, list):
        artifacts = []
        errors.append("firmware artifact manifest artifacts must be a list")
    artifact_count = artifact_manifest.get("artifact_count")
    if not isinstance(artifact_count, int):
        errors.append("firmware artifact manifest artifact_count must be an integer")
        artifact_count = len(artifacts)
    elif artifact_count != len(artifacts):
        errors.append(
            "firmware artifact manifest artifact_count "
            f"{artifact_count} does not match artifacts length {len(artifacts)}"
        )
    artifacts_by_path = {
        str(artifact.get("path", "")): artifact
        for artifact in artifacts
        if isinstance(artifact, dict)
    }
    if len(artifacts_by_path) != len(artifacts):
        errors.append("firmware artifact manifest artifacts must be objects with unique paths")

    known_specs_by_path = {spec.path: spec for spec in firmware_artifact_specs.ARTIFACTS}
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            continue
        path = str(artifact.get("path", "")).strip()
        size = artifact.get("size")
        sha256 = str(artifact.get("sha256", "")).strip()
        if not path:
            errors.append("firmware artifact manifest artifact path must be present")
        if not isinstance(size, int) or size <= 0:
            errors.append(f"firmware artifact manifest {path or '<missing path>'} size must be positive")
        if not SHA256_RE.fullmatch(sha256):
            errors.append(
                f"firmware artifact manifest {path or '<missing path>'} sha256 must be a SHA256 hex string"
            )
        expected_spec = known_specs_by_path.get(path)
        if expected_spec is not None:
            for field, expected_value in [
                ("role", expected_spec.role),
                ("side", expected_spec.side),
                ("kind", expected_spec.kind),
            ]:
                if artifact.get(field) != expected_value:
                    errors.append(
                        f"firmware artifact manifest {path} {field} must be {expected_value}"
                    )
        if path:
            artifact_path = Path(path)
            if artifact_path.is_absolute():
                errors.append(f"firmware artifact manifest {path} path must be relative")
            else:
                artifact_path = (resolved_artifact_root / artifact_path).resolve(strict=False)
                try:
                    artifact_path.relative_to(resolved_artifact_root)
                except ValueError:
                    errors.append(
                        f"firmware artifact manifest {path} path must stay inside artifact root"
                    )
                    continue
                try:
                    actual_size = artifact_path.stat().st_size
                except OSError as e:
                    errors.append(
                        f"firmware artifact manifest {path} file is not readable: {e}"
                    )
                else:
                    if isinstance(size, int) and actual_size != size:
                        errors.append(
                            f"firmware artifact manifest {path} size {size} "
                            f"does not match file size {actual_size}"
                        )
                    if SHA256_RE.fullmatch(sha256):
                        actual_sha256 = firmware_artifact_manifest.sha256_file(artifact_path)
                        if actual_sha256 != sha256:
                            errors.append(
                                f"firmware artifact manifest {path} sha256 {sha256!r} "
                                f"does not match file {actual_sha256!r}"
                            )

    pair_digest_payload = json.dumps(
        [
            {
                "path": artifact.get("path"),
                "size": artifact.get("size"),
                "sha256": artifact.get("sha256"),
            }
            for artifact in artifacts
            if isinstance(artifact, dict)
        ],
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    expected_pair_sha256 = hashlib.sha256(pair_digest_payload).hexdigest()
    if pair_sha256 and pair_sha256 != expected_pair_sha256:
        errors.append(
            "firmware artifact manifest pair_sha256 "
            f"{pair_sha256!r} does not match artifact entries {expected_pair_sha256!r}"
        )

    required_artifact_specs = firmware_artifact_specs.HARDWARE_VALIDATION_REQUIRED_ARTIFACTS
    required_uf2_paths = [spec.path for spec in required_artifact_specs]
    for spec in required_artifact_specs:
        artifact = artifacts_by_path.get(spec.path)
        if artifact is None:
            errors.append(
                f"firmware artifact manifest missing required {spec.kind} {spec.path}"
            )
            continue

    return {
        "path": str(artifact_manifest_path),
        "firmware_ref": firmware_ref,
        "artifact_count": artifact_count,
        "pair_sha256": pair_sha256,
        "required_uf2_paths": required_uf2_paths,
        "errors": errors,
    }


def build_status(args: argparse.Namespace) -> MigrationStatus:
    software = software_status(
        args.porting_manifest,
        args.keyboard_toml,
        args.zmk_keymap,
        args.require_zmk_source,
        args.require_zmk_clean_source,
        args.require_zmk_source_commit,
        args.coverage_baseline,
    )
    firmware_artifacts = firmware_artifact_status(
        args.firmware_artifact_manifest,
        args.require_firmware_ref,
        args.artifact_root,
    )
    artifact_errors = firmware_artifacts["errors"] if firmware_artifacts else []
    if args.require_hardware_validated and firmware_artifacts is None:
        artifact_errors = artifact_errors + [
            "firmware artifact manifest is required when --require-hardware-validated is used"
        ]
    required_firmware_ref = args.require_firmware_ref
    if (
        required_firmware_ref is None
        and firmware_artifacts is not None
        and firmware_artifacts["firmware_ref"]
        and not hardware_validation.is_mutable_firmware_ref(firmware_artifacts["firmware_ref"])
    ):
        required_firmware_ref = firmware_artifacts["firmware_ref"]
    required_artifact_pair_sha256 = (
        firmware_artifacts["pair_sha256"]
        if firmware_artifacts is not None and not artifact_errors
        else None
    )
    hardware = hardware_status(
        args.hardware_manifest,
        args.hardware_baseline,
        args.evidence,
        args.evidence_artifact_root,
        required_firmware_ref,
        required_artifact_pair_sha256,
        args.require_evidence_inventory or args.require_hardware_validated,
        args.require_evidence_artifact_paths or args.require_hardware_validated,
        artifact_errors,
    )
    hardware_classified = bool(hardware["classified"])
    hardware_validated = hardware_classified and hardware["validated"] == hardware["total"]
    ready_without_hardware = software.complete and software.zmk_source_clean and hardware_classified
    return MigrationStatus(
        software=software,
        hardware=hardware,
        firmware_artifacts=firmware_artifacts,
        ready_for_release_without_hardware=ready_without_hardware,
        fully_validated=ready_without_hardware and hardware_validated,
    )


def as_json(status: MigrationStatus) -> dict[str, Any]:
    return {
        "software": {
            "passed": status.software.passed,
            "total": status.software.total,
            "rate": status.software.rate,
            "zmk_source": status.software.zmk_source,
            "zmk_source_clean": status.software.zmk_source_clean,
            "zmk_source_clean_errors": status.software.zmk_source_clean_errors,
            "by_kind": status.software.by_kind,
            "implementation": status.software.implementation,
            "complete": status.software.complete,
            "failed": status.software.failed,
        },
        "hardware": status.hardware,
        "firmware_artifacts": status.firmware_artifacts,
        "ready_for_release_without_hardware": status.ready_for_release_without_hardware,
        "fully_validated": status.fully_validated,
    }


def print_text(status: MigrationStatus) -> None:
    implementation = status.software.implementation
    hardware = status.hardware
    hardware_rate = "n/a" if hardware["rate"] is None else f"{hardware['rate']:.2f}%"
    print("Migration status:")
    print(
        "Software coverage: "
        f"{status.software.passed}/{status.software.total} = {percent(status.software.rate)}"
    )
    dirty = status.software.zmk_source.get("git_dirty")
    dirty_text = "unknown" if dirty is None else "yes" if dirty else "no"
    dirty_paths = status.software.zmk_source.get("git_dirty_paths") or []
    dirty_paths_text = ",".join(str(path) for path in dirty_paths) if dirty_paths else "none"
    print(
        "ZMK source: "
        f"path={status.software.zmk_source.get('keymap_path') or 'n/a'} "
        f"available={'yes' if status.software.zmk_source.get('available') else 'no'} "
        f"repo={status.software.zmk_source.get('repo_path') or 'n/a'} "
        f"git_commit={status.software.zmk_source.get('git_commit') or 'n/a'} "
        f"dirty={dirty_text} "
        f"dirty_paths={dirty_paths_text}"
    )
    print(
        "Software implementation: "
        f"{implementation['implemented']}/{implementation['total']} = "
        f"{percent(implementation['rate'])}"
    )
    print(f"ZMK source clean: {'pass' if status.software.zmk_source_clean else 'fail'}")
    print(
        "Hardware validation: "
        f"{hardware['validated']}/{hardware['total']} = {hardware_rate}"
    )
    print(f"Hardware check inventory SHA256: {hardware['check_inventory_sha256']}")
    if status.firmware_artifacts is not None:
        artifacts = status.firmware_artifacts
        artifact_status = "pass" if not artifacts["errors"] else "fail"
        print(
            "Firmware artifacts: "
            f"{artifact_status} ref={artifacts['firmware_ref'] or 'n/a'} "
            f"count={artifacts['artifact_count']} pair_sha256={artifacts['pair_sha256'] or 'n/a'}"
        )
    print(
        "Release gate without hardware: "
        f"{'pass' if status.ready_for_release_without_hardware else 'fail'}"
    )
    print(f"Full validation: {'pass' if status.fully_validated else 'fail'}")
    if status.software.failed:
        print("Software failures:")
        for failure in status.software.failed:
            print(
                f"- {failure['kind']} {failure['id']}: "
                f"{failure['passed']}/{failure['total']} {failure['message']}"
            )
    if not status.software.zmk_source_clean and not any(
        failure["id"] == "zmk_source_clean" for failure in status.software.failed
    ):
        print("ZMK source clean failures:")
        for failure in status.software.zmk_source_clean_errors:
            print(f"- {failure}")
    if hardware["errors"]:
        print("Hardware validation failures:")
        for error in hardware["errors"]:
            print(f"- {error}")
    if hardware["remaining"]:
        print("Hardware remaining:")
        for item in hardware["remaining"]:
            needs = item.get("evidence_needles", "")
            artifacts = item.get("evidence_artifacts", "")
            artifact_paths = item.get("artifact_paths", "")
            suffix_parts = []
            if artifacts:
                suffix_parts.append(f"artifacts: {artifacts}")
            if needs:
                suffix_parts.append(f"needs: {needs}")
            if artifact_paths:
                suffix_parts.append(f"artifact_paths: {artifact_paths}")
            suffix = f" [{'; '.join(suffix_parts)}]" if suffix_parts else ""
            print(f"- {item['id']} ({item['area']}/{item['side']}): {item['status']}{suffix}")


def markdown_table(rows: list[list[Any]]) -> str:
    escaped_rows = [
        [hardware_validation.markdown_escape(cell) for cell in row] for row in rows
    ]
    header = escaped_rows[0]
    separator = ["---"] * len(header)
    lines = [
        "| " + " | ".join(header) + " |",
        "| " + " | ".join(separator) + " |",
    ]
    for row in escaped_rows[1:]:
        lines.append("| " + " | ".join(row) + " |")
    return "\n".join(lines)


def progress_rows(label: str, group: dict[str, Any]) -> list[list[str]]:
    rows = [[label, "Validated", "Total", "Rate"]]
    for key, progress in group.items():
        rate = progress.get("rate")
        rows.append(
            [
                str(key or f"unknown {label.lower()}"),
                str(progress.get("validated", 0)),
                str(progress.get("total", 0)),
                "n/a" if rate is None else f"{rate:.2f}%",
            ]
        )
    return rows


def print_markdown(status: MigrationStatus) -> None:
    implementation = status.software.implementation
    hardware = status.hardware
    hardware_rate = "n/a" if hardware["rate"] is None else f"{hardware['rate']:.2f}%"
    print("## RMK Migration Status")
    print()
    print(
        markdown_table(
            [
                ["Gate", "Passed", "Total", "Rate"],
                [
                    "Software coverage",
                    str(status.software.passed),
                    str(status.software.total),
                    percent(status.software.rate),
                ],
                [
                    "Software implementation",
                    str(implementation["implemented"]),
                    str(implementation["total"]),
                    percent(implementation["rate"]),
                ],
                [
                    "Hardware validation",
                    str(hardware["validated"]),
                    str(hardware["total"]),
                    hardware_rate,
                ],
            ]
        )
    )
    print()
    dirty = status.software.zmk_source.get("git_dirty")
    dirty_text = "unknown" if dirty is None else "yes" if dirty else "no"
    dirty_paths = status.software.zmk_source.get("git_dirty_paths") or []
    dirty_paths_text = ",".join(str(path) for path in dirty_paths) if dirty_paths else "none"
    print(
        "ZMK source: "
        f"`{hardware_validation.markdown_escape(status.software.zmk_source.get('keymap_path') or 'n/a')}`; "
        f"available={'yes' if status.software.zmk_source.get('available') else 'no'}; "
        f"repo=`{hardware_validation.markdown_escape(status.software.zmk_source.get('repo_path') or 'n/a')}`; "
        f"git_commit=`{hardware_validation.markdown_escape(status.software.zmk_source.get('git_commit') or 'n/a')}`; "
        f"dirty={dirty_text}; "
        f"dirty_paths=`{hardware_validation.markdown_escape(dirty_paths_text)}`"
    )
    print()
    print(
        "Hardware check inventory SHA256: "
        f"`{hardware_validation.markdown_escape(hardware['check_inventory_sha256'])}`"
    )
    print()
    print(f"ZMK source clean: {'pass' if status.software.zmk_source_clean else 'fail'}")
    if not status.software.zmk_source_clean:
        for failure in status.software.zmk_source_clean_errors:
            print(f"- {hardware_validation.markdown_escape(failure)}")
        print()
    print(
        f"Release gate without hardware: "
        f"{'pass' if status.ready_for_release_without_hardware else 'fail'}"
    )
    print()
    print(f"Full validation: {'pass' if status.fully_validated else 'fail'}")
    if status.firmware_artifacts is not None:
        artifacts = status.firmware_artifacts
        print()
        print(
            markdown_table(
                [
                    ["Firmware artifact manifest", "Firmware ref", "Artifact count", "Pair SHA256"],
                    [
                        artifacts["path"],
                        artifacts["firmware_ref"] or "n/a",
                        str(artifacts["artifact_count"]),
                        artifacts["pair_sha256"] or "n/a",
                    ],
                ]
            )
        )
    if status.software.failed:
        print()
        print("### Software Failures")
        print()
        for failure in status.software.failed:
            kind = hardware_validation.markdown_escape(failure["kind"])
            failure_id = hardware_validation.markdown_escape(failure["id"])
            message = hardware_validation.markdown_escape(failure["message"])
            print(
                f"- `{kind}` `{failure_id}`: "
                f"{failure['passed']}/{failure['total']} {message}"
            )
    if hardware["errors"]:
        print()
        print("### Hardware Validation Failures")
        print()
        for error in hardware["errors"]:
            print(f"- {hardware_validation.markdown_escape(error)}")
    if hardware["by_area"]:
        print()
        print("### Hardware Progress By Area")
        print()
        print(markdown_table(progress_rows("Area", hardware["by_area"])))
    if hardware["by_side"]:
        print()
        print("### Hardware Progress By Side")
        print()
        print(markdown_table(progress_rows("Side", hardware["by_side"])))
    if hardware["remaining"]:
        print()
        print("### Hardware Remaining")
        print()
        rows = [
            [
                "ID",
                "Area",
                "Side",
                "Status",
                "Required artifacts",
                "Required observations",
                "Evidence artifact paths",
            ]
        ]
        rows.extend(
            [
                str(item["id"]),
                str(item["area"]),
                str(item["side"]),
                str(item["status"]),
                str(item.get("evidence_artifacts", "")),
                str(item.get("evidence_needles", "")),
                str(item.get("artifact_paths", "")),
            ]
            for item in hardware["remaining"]
        )
        print(markdown_table(rows))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--porting-manifest",
        type=Path,
        default=Path("tools/porting_coverage_manifest.toml"),
    )
    parser.add_argument("--keyboard-toml", type=Path, default=Path("keyboard.toml"))
    parser.add_argument("--zmk-keymap", type=Path, default=None)
    parser.add_argument("--require-zmk-source", action="store_true")
    parser.add_argument("--require-zmk-clean-source", action="store_true")
    parser.add_argument("--require-zmk-source-commit", action="store_true")
    parser.add_argument("--coverage-baseline", type=Path, default=None)
    parser.add_argument(
        "--hardware-manifest",
        type=Path,
        default=Path("tools/hardware_validation_manifest.toml"),
    )
    parser.add_argument("--hardware-baseline", type=Path, default=None)
    parser.add_argument("--evidence", type=Path, action="append", default=[])
    parser.add_argument(
        "--firmware-artifact-manifest",
        type=Path,
        default=None,
        help="validate hardware evidence against a generated firmware artifact hash manifest",
    )
    parser.add_argument(
        "--artifact-root",
        type=Path,
        default=Path("."),
        help="directory used to resolve relative paths in --firmware-artifact-manifest",
    )
    parser.add_argument(
        "--evidence-artifact-root",
        type=Path,
        default=Path("."),
        help="directory used to resolve relative artifact_paths entries in evidence overlays",
    )
    parser.add_argument("--require-firmware-ref", metavar="REF")
    parser.add_argument("--require-software-complete", action="store_true")
    parser.add_argument("--require-hardware-classified", action="store_true")
    parser.add_argument("--require-release-ready", action="store_true")
    parser.add_argument("--require-evidence-artifact-paths", action="store_true")
    parser.add_argument(
        "--require-evidence-inventory",
        action="store_true",
        help=(
            "fail unless every evidence overlay declares the current "
            "hardware_check_inventory_sha256 metadata"
        ),
    )
    parser.add_argument("--require-hardware-validated", action="store_true")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--markdown", action="store_true")
    args = parser.parse_args()

    if args.json and args.markdown:
        parser.error("--json and --markdown are mutually exclusive")
    if args.require_firmware_ref and hardware_validation.is_mutable_firmware_ref(
        args.require_firmware_ref
    ):
        parser.error("--require-firmware-ref must be an immutable flashed tag or commit")

    status = build_status(args)
    if args.json:
        print(json.dumps(as_json(status), indent=2, sort_keys=True))
    elif args.markdown:
        print_markdown(status)
    else:
        print_text(status)

    if args.require_software_complete and not status.software.complete:
        raise SystemExit(1)
    if args.require_zmk_clean_source and any(
        failure["id"] == "zmk_source_clean" for failure in status.software.failed
    ):
        raise SystemExit(1)
    if args.require_zmk_source_commit and any(
        failure["id"] == "zmk_source_commit" for failure in status.software.failed
    ):
        print("ZMK source commit does not match the migration contract:", file=sys.stderr)
        for failure in status.software.failed:
            if failure["id"] == "zmk_source_commit":
                print(f"- {failure['message']}", file=sys.stderr)
        raise SystemExit(1)
    if args.require_hardware_classified and not status.hardware["classified"]:
        raise SystemExit(1)
    if args.require_evidence_artifact_paths and not status.hardware["classified"]:
        raise SystemExit(1)
    if args.require_evidence_inventory and not status.hardware["classified"]:
        raise SystemExit(1)
    if args.require_firmware_ref is not None and not status.hardware["classified"]:
        raise SystemExit(1)
    if args.require_release_ready and not status.ready_for_release_without_hardware:
        raise SystemExit(1)
    if args.require_hardware_validated and not status.fully_validated:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
