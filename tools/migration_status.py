#!/usr/bin/env python3
"""Report combined ZMK-to-RMK porting and hardware validation status."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import hardware_validation
import porting_coverage


@dataclass
class SoftwareStatus:
    passed: int
    total: int
    rate: float | None
    by_kind: dict[str, dict[str, int | float | None]]
    implementation: dict[str, Any]
    complete: bool
    failed: list[dict[str, str | int]]


@dataclass
class MigrationStatus:
    software: SoftwareStatus
    hardware: dict[str, Any]
    ready_for_release_without_hardware: bool
    fully_validated: bool


def percent(rate: float | None) -> str:
    return "n/a" if rate is None else f"{rate * 100.0:.2f}%"


def software_status(
    manifest_path: Path,
    keyboard_path: Path,
    zmk_keymap_path: Path | None,
    require_zmk_source: bool,
) -> SoftwareStatus:
    results = porting_coverage.run(
        manifest_path,
        keyboard_path,
        zmk_keymap_path,
        require_zmk_source,
    )
    manifest = porting_coverage.load_toml(manifest_path)
    implementation = porting_coverage.porting_status_summary(manifest)
    passed = sum(result.passed for result in results)
    total = sum(result.total for result in results)
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
    ]
    by_kind = {
        kind: {
            "passed": bucket.passed,
            "total": bucket.total,
            "rate": bucket.rate,
        }
        for kind, bucket in porting_coverage.coverage_by_kind(results).items()
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
        by_kind=by_kind,
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
    evidence_paths: list[Path],
    required_firmware_ref: str | None,
) -> dict[str, Any]:
    manifest, evidence_errors = hardware_validation.merge_evidence(
        hardware_validation.load_toml(manifest_path),
        [hardware_validation.load_toml(path) for path in evidence_paths],
    )
    summary = hardware_validation.summarize(
        manifest,
        evidence_errors,
        Path("."),
        required_firmware_ref,
    )
    return hardware_validation.as_json(summary)


def build_status(args: argparse.Namespace) -> MigrationStatus:
    software = software_status(
        args.porting_manifest,
        args.keyboard_toml,
        args.zmk_keymap,
        args.require_zmk_source,
    )
    hardware = hardware_status(
        args.hardware_manifest,
        args.evidence,
        args.require_firmware_ref,
    )
    hardware_classified = bool(hardware["classified"])
    hardware_validated = hardware_classified and hardware["validated"] == hardware["total"]
    ready_without_hardware = software.complete and hardware_classified
    return MigrationStatus(
        software=software,
        hardware=hardware,
        ready_for_release_without_hardware=ready_without_hardware,
        fully_validated=ready_without_hardware and hardware_validated,
    )


def as_json(status: MigrationStatus) -> dict[str, Any]:
    return {
        "software": {
            "passed": status.software.passed,
            "total": status.software.total,
            "rate": status.software.rate,
            "by_kind": status.software.by_kind,
            "implementation": status.software.implementation,
            "complete": status.software.complete,
            "failed": status.software.failed,
        },
        "hardware": status.hardware,
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
    print(
        "Software implementation: "
        f"{implementation['implemented']}/{implementation['total']} = "
        f"{percent(implementation['rate'])}"
    )
    print(
        "Hardware validation: "
        f"{hardware['validated']}/{hardware['total']} = {hardware_rate}"
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
    if hardware["remaining"]:
        print("Hardware remaining:")
        for item in hardware["remaining"]:
            print(f"- {item['id']} ({item['area']}/{item['side']}): {item['status']}")


def markdown_table(rows: list[list[str]]) -> str:
    header = rows[0]
    separator = ["---"] * len(header)
    lines = [
        "| " + " | ".join(header) + " |",
        "| " + " | ".join(separator) + " |",
    ]
    for row in rows[1:]:
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
    print(
        f"Release gate without hardware: "
        f"{'pass' if status.ready_for_release_without_hardware else 'fail'}"
    )
    print()
    print(f"Full validation: {'pass' if status.fully_validated else 'fail'}")
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
        rows = [["ID", "Area", "Side", "Status"]]
        rows.extend(
            [
                str(item["id"]),
                str(item["area"]),
                str(item["side"]),
                str(item["status"]),
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
    parser.add_argument(
        "--hardware-manifest",
        type=Path,
        default=Path("tools/hardware_validation_manifest.toml"),
    )
    parser.add_argument("--evidence", type=Path, action="append", default=[])
    parser.add_argument("--require-firmware-ref", metavar="REF")
    parser.add_argument("--require-software-complete", action="store_true")
    parser.add_argument("--require-hardware-classified", action="store_true")
    parser.add_argument("--require-hardware-validated", action="store_true")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--markdown", action="store_true")
    args = parser.parse_args()

    if args.json and args.markdown:
        parser.error("--json and --markdown are mutually exclusive")

    status = build_status(args)
    if args.json:
        print(json.dumps(as_json(status), indent=2, sort_keys=True))
    elif args.markdown:
        print_markdown(status)
    else:
        print_text(status)

    if args.require_software_complete and not status.software.complete:
        raise SystemExit(1)
    if args.require_hardware_classified and not status.hardware["classified"]:
        raise SystemExit(1)
    if args.require_firmware_ref is not None and not status.hardware["classified"]:
        raise SystemExit(1)
    if args.require_hardware_validated and not status.fully_validated:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
