#!/usr/bin/env python3
"""Summarize real-hardware validation coverage for the RMK port."""

from __future__ import annotations

import argparse
import copy
import json
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any


VALID_STATUSES = frozenset(
    {
        "requires_hardware",
        "validated",
        "simulated",
        "blocked",
    }
)
VALIDATED_STATUSES = frozenset({"validated"})
REQUIRED_FIELDS = ("id", "area", "side", "requirement", "evidence", "source", "status")
VALIDATED_EVIDENCE_FIELDS = ("validated_at", "tester", "artifact_or_notes")
EVIDENCE_UPDATE_FIELDS = ("status", "validated_at", "tester", "artifact_or_notes")


@dataclass
class HardwareValidationSummary:
    total: int
    validated: int
    by_status: dict[str, int]
    remaining: list[dict[str, str]]
    errors: list[str]

    @property
    def rate(self) -> float | None:
        if self.total == 0:
            return None
        return self.validated / self.total * 100

    @property
    def classified(self) -> bool:
        return not self.errors


def load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as f:
        return tomllib.load(f)


def merge_evidence(
    manifest: dict[str, Any], evidence_docs: list[dict[str, Any]]
) -> tuple[dict[str, Any], list[str]]:
    merged = copy.deepcopy(manifest)
    checks = merged.get("checks", [])
    errors: list[str] = []
    if not isinstance(checks, list):
        return merged, errors

    by_id: dict[str, dict[str, Any]] = {}
    for check in checks:
        if isinstance(check, dict) and isinstance(check.get("id"), str):
            by_id[check["id"]] = check

    seen_evidence: set[str] = set()
    for evidence_doc in evidence_docs:
        evidence_entries = evidence_doc.get("evidence", [])
        if not isinstance(evidence_entries, list):
            errors.append("evidence must be an array")
            continue
        for index, entry in enumerate(evidence_entries):
            if not isinstance(entry, dict):
                errors.append(f"evidence #{index + 1} must be a table")
                continue
            check_id = str(entry.get("id", ""))
            if not check_id:
                errors.append(f"evidence #{index + 1}: missing required field id")
                continue
            if check_id in seen_evidence:
                errors.append(f"{check_id}: duplicate evidence entry")
                continue
            seen_evidence.add(check_id)
            if check_id not in by_id:
                errors.append(f"{check_id}: evidence references unknown hardware check")
                continue
            if "status" not in entry:
                errors.append(f"{check_id}: evidence entry must include status")
                continue
            for field in EVIDENCE_UPDATE_FIELDS:
                if field in entry:
                    by_id[check_id][field] = entry[field]

    return merged, errors


def summarize(
    manifest: dict[str, Any], initial_errors: list[str] | None = None
) -> HardwareValidationSummary:
    checks = manifest.get("checks", [])
    by_status = {status: 0 for status in sorted(VALID_STATUSES)}
    remaining: list[dict[str, str]] = []
    errors = list(initial_errors or [])
    seen_ids: set[str] = set()
    validated = 0

    if not isinstance(checks, list):
        return HardwareValidationSummary(0, 0, by_status, [], ["checks must be an array"])
    if not checks:
        errors.append("checks must contain at least one hardware validation item")

    for index, check in enumerate(checks):
        if not isinstance(check, dict):
            errors.append(f"check #{index + 1} must be a table")
            continue

        check_id = str(check.get("id", f"#{index + 1}"))
        missing = [field for field in REQUIRED_FIELDS if not str(check.get(field, "")).strip()]
        if missing:
            errors.append(f"{check_id}: missing required field(s): {', '.join(missing)}")

        if check_id in seen_ids:
            errors.append(f"{check_id}: duplicate check id")
        seen_ids.add(check_id)

        status = str(check.get("status", ""))
        if status not in VALID_STATUSES:
            errors.append(f"{check_id}: invalid status {status!r}")
        else:
            by_status[status] += 1
            if status in VALIDATED_STATUSES:
                missing_evidence = [
                    field
                    for field in VALIDATED_EVIDENCE_FIELDS
                    if not str(check.get(field, "")).strip()
                ]
                if missing_evidence:
                    errors.append(
                        f"{check_id}: validated checks require evidence field(s): "
                        f"{', '.join(missing_evidence)}"
                    )
                else:
                    validated += 1
            else:
                remaining.append(
                    {
                        "id": check_id,
                        "area": str(check.get("area", "")),
                        "side": str(check.get("side", "")),
                        "status": status,
                        "evidence": str(check.get("evidence", "")),
                    }
                )

    return HardwareValidationSummary(
        total=len(checks),
        validated=validated,
        by_status=by_status,
        remaining=remaining,
        errors=errors,
    )


def as_json(summary: HardwareValidationSummary) -> dict[str, Any]:
    return {
        "total": summary.total,
        "validated": summary.validated,
        "rate": summary.rate,
        "by_status": summary.by_status,
        "remaining": summary.remaining,
        "errors": summary.errors,
        "classified": summary.classified,
    }


def markdown_escape(value: Any) -> str:
    return str(value).replace("|", "\\|").replace("\n", " ")


def as_markdown(manifest: dict[str, Any], summary: HardwareValidationSummary) -> str:
    if summary.rate is None:
        headline = "Hardware validation: 0/0 = n/a"
    else:
        headline = (
            f"Hardware validation: {summary.validated}/{summary.total} = "
            f"{summary.rate:.2f}% validated"
        )
    status_counts = ", ".join(
        f"`{status}`={count}" for status, count in summary.by_status.items() if count
    )
    lines = [
        "## Real-Hardware Validation",
        "",
        headline,
        "",
        f"Status: {status_counts or 'none'}",
        "",
        "| ID | Area | Side | Status | Requirement | Required evidence | Validated at | Tester | Artifact/notes |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for check in manifest.get("checks", []):
        if not isinstance(check, dict):
            continue
        lines.append(
            "| {id} | {area} | {side} | `{status}` | {requirement} | {evidence} | {validated_at} | {tester} | {artifact_or_notes} |".format(
                id=markdown_escape(check.get("id", "")),
                area=markdown_escape(check.get("area", "")),
                side=markdown_escape(check.get("side", "")),
                status=markdown_escape(check.get("status", "")),
                requirement=markdown_escape(check.get("requirement", "")),
                evidence=markdown_escape(check.get("evidence", "")),
                validated_at=markdown_escape(check.get("validated_at", "")),
                tester=markdown_escape(check.get("tester", "")),
                artifact_or_notes=markdown_escape(check.get("artifact_or_notes", "")),
            )
        )
    if summary.errors:
        lines.extend(["", "### Manifest Errors", ""])
        lines.extend(f"- {markdown_escape(error)}" for error in summary.errors)
    return "\n".join(lines) + "\n"


def print_text(summary: HardwareValidationSummary) -> None:
    if summary.rate is None:
        print("Hardware validation: 0/0 = n/a")
    else:
        print(
            f"Hardware validation: {summary.validated}/{summary.total} = "
            f"{summary.rate:.2f}% validated"
        )
    status_counts = ", ".join(
        f"{status}={count}" for status, count in summary.by_status.items() if count
    )
    print(f"Hardware validation status: {status_counts or 'none'}")
    if summary.remaining:
        print("Hardware validation remaining:")
        for item in summary.remaining:
            print(
                f"- {item['id']} ({item['area']}/{item['side']}): "
                f"{item['status']} - {item['evidence']}"
            )
    if summary.errors:
        print("Hardware validation manifest errors:", file=sys.stderr)
        for error in summary.errors:
            print(f"- {error}", file=sys.stderr)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("tools/hardware_validation_manifest.toml"),
    )
    parser.add_argument(
        "--evidence",
        type=Path,
        action="append",
        default=[],
        help="overlay real-hardware evidence entries before calculating validation status",
    )
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--markdown", action="store_true")
    parser.add_argument(
        "--require-classified",
        action="store_true",
        help="fail if any hardware validation check is malformed or unclassified",
    )
    parser.add_argument(
        "--require-validated",
        action="store_true",
        help="fail until every real-hardware check has status validated",
    )
    args = parser.parse_args()

    manifest, evidence_errors = merge_evidence(
        load_toml(args.manifest), [load_toml(path) for path in args.evidence]
    )
    summary = summarize(manifest, evidence_errors)
    if args.json and args.markdown:
        parser.error("--json and --markdown are mutually exclusive")
    if args.json:
        print(json.dumps(as_json(summary), indent=2, sort_keys=True))
    elif args.markdown:
        print(as_markdown(manifest, summary), end="")
    else:
        print_text(summary)

    if args.require_classified and not summary.classified:
        raise SystemExit(1)
    if args.require_validated and (not summary.classified or summary.validated != summary.total):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
