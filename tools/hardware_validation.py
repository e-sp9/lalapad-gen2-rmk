#!/usr/bin/env python3
"""Summarize real-hardware validation coverage for the RMK port."""

from __future__ import annotations

import argparse
import copy
import json
import re
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
VALIDATED_EVIDENCE_FIELDS = ("validated_at", "tester", "firmware_ref", "artifact_or_notes")
EVIDENCE_UPDATE_FIELDS = (
    "status",
    "validated_at",
    "tester",
    "firmware_ref",
    "artifact_or_notes",
)
MARKDOWN_HEADING_RE = re.compile(r"^\s{0,3}#{1,6}\s+(.+?)\s*#*\s*$")


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


def markdown_anchor(heading: str) -> str:
    text = re.sub(r"`([^`]*)`", r"\1", heading)
    text = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", text)
    text = re.sub(r"<[^>]+>", "", text)
    text = re.sub(r"[^\w\s-]", "", text.lower(), flags=re.UNICODE)
    return re.sub(r"\s+", "-", text.strip())


def markdown_anchors(path: Path) -> set[str]:
    anchors: set[str] = set()
    seen: dict[str, int] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        match = MARKDOWN_HEADING_RE.match(line)
        if not match:
            continue
        base = markdown_anchor(match.group(1))
        if not base:
            continue
        index = seen.get(base, 0)
        anchors.add(base if index == 0 else f"{base}-{index}")
        seen[base] = index + 1
    return anchors


def validate_source_ref(check_id: str, source: str, source_root: Path) -> list[str]:
    errors: list[str] = []
    source_path_text, separator, anchor = source.partition("#")
    if not source_path_text:
        return [f"{check_id}: source must include a file path"]

    root = source_root.resolve()
    source_path = (root / source_path_text).resolve()
    try:
        source_path.relative_to(root)
    except ValueError:
        return [f"{check_id}: source path must stay inside {source_root}"]

    if not source_path.is_file():
        return [f"{check_id}: source file {source_path_text!r} does not exist"]
    if source_path.suffix.lower() not in {".md", ".markdown"}:
        return [f"{check_id}: source file {source_path_text!r} must be Markdown"]
    if separator and not anchor:
        return [f"{check_id}: source anchor must not be empty"]
    if anchor:
        anchors = markdown_anchors(source_path)
        if anchor not in anchors:
            errors.append(
                f"{check_id}: source anchor #{anchor} was not found in {source_path_text!r}"
            )
    return errors


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
    manifest: dict[str, Any],
    initial_errors: list[str] | None = None,
    source_root: Path | None = None,
    required_firmware_ref: str | None = None,
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
        source = str(check.get("source", "")).strip()
        if source:
            errors.extend(validate_source_ref(check_id, source, source_root or Path(".")))

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
                elif (
                    required_firmware_ref is not None
                    and str(check.get("firmware_ref", "")) != required_firmware_ref
                ):
                    errors.append(
                        f"{check_id}: validated firmware_ref "
                        f"{str(check.get('firmware_ref', ''))!r} does not match "
                        f"required {required_firmware_ref!r}"
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


def toml_string(value: Any) -> str:
    return json.dumps(str(value))


def toml_comment(value: Any) -> list[str]:
    lines = str(value).splitlines() or [""]
    return [f"# {line}" if line else "#" for line in lines]


def as_evidence_template(manifest: dict[str, Any]) -> str:
    lines = [
        "# Hardware validation evidence overlay.",
        "# Fill this file after testing real hardware, then run:",
        "#",
        "#   python3 tools/hardware_validation.py --evidence path/to/evidence.toml --markdown",
        "#   python3 tools/hardware_validation.py --evidence path/to/evidence.toml --require-validated",
        "#   python3 tools/hardware_validation.py --evidence path/to/evidence.toml --require-firmware-ref <tag-or-commit>",
        "#",
        "# Entries are keyed by id from tools/hardware_validation_manifest.toml.",
        "# Change status to \"validated\" only when validated_at, tester, firmware_ref, and artifact_or_notes are filled.",
        "",
    ]
    checks = manifest.get("checks", [])
    if isinstance(checks, list):
        for check in checks:
            if not isinstance(check, dict):
                continue
            lines.append("[[evidence]]")
            lines.append(f"id = {toml_string(check.get('id', ''))}")
            lines.append('status = "requires_hardware"')
            lines.append('validated_at = ""')
            lines.append('tester = ""')
            lines.append('firmware_ref = ""')
            lines.append('artifact_or_notes = ""')
            lines.append("# Requirement:")
            lines.extend(toml_comment(check.get("requirement", "")))
            lines.append("# Required evidence:")
            lines.extend(toml_comment(check.get("evidence", "")))
            lines.append("")
    return "\n".join(lines)


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
        "| ID | Area | Side | Status | Requirement | Required evidence | Validated at | Tester | Firmware ref | Artifact/notes |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for check in manifest.get("checks", []):
        if not isinstance(check, dict):
            continue
        lines.append(
            "| {id} | {area} | {side} | `{status}` | {requirement} | {evidence} | {validated_at} | {tester} | {firmware_ref} | {artifact_or_notes} |".format(
                id=markdown_escape(check.get("id", "")),
                area=markdown_escape(check.get("area", "")),
                side=markdown_escape(check.get("side", "")),
                status=markdown_escape(check.get("status", "")),
                requirement=markdown_escape(check.get("requirement", "")),
                evidence=markdown_escape(check.get("evidence", "")),
                validated_at=markdown_escape(check.get("validated_at", "")),
                tester=markdown_escape(check.get("tester", "")),
                firmware_ref=markdown_escape(check.get("firmware_ref", "")),
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
        "--evidence-template",
        action="store_true",
        help="print an evidence overlay template containing every manifest check",
    )
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
    parser.add_argument(
        "--require-firmware-ref",
        help="fail if any validated hardware evidence was captured against a different firmware tag or commit",
    )
    args = parser.parse_args()

    manifest, evidence_errors = merge_evidence(
        load_toml(args.manifest), [load_toml(path) for path in args.evidence]
    )
    summary = summarize(manifest, evidence_errors, Path("."), args.require_firmware_ref)
    output_modes = sum(bool(mode) for mode in [args.json, args.markdown, args.evidence_template])
    if output_modes > 1:
        parser.error("--json, --markdown, and --evidence-template are mutually exclusive")
    if args.json:
        print(json.dumps(as_json(summary), indent=2, sort_keys=True))
    elif args.markdown:
        print(as_markdown(manifest, summary), end="")
    elif args.evidence_template:
        print(as_evidence_template(manifest), end="")
    else:
        print_text(summary)

    if args.require_classified and not summary.classified:
        raise SystemExit(1)
    if args.require_firmware_ref is not None and not summary.classified:
        raise SystemExit(1)
    if args.require_validated and (not summary.classified or summary.validated != summary.total):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
