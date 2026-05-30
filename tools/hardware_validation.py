#!/usr/bin/env python3
"""Summarize real-hardware validation coverage for the RMK port."""

from __future__ import annotations

import argparse
import copy
import datetime
import hashlib
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
EVIDENCE_INVENTORY_FIELD = "hardware_check_inventory_sha256"
PLACEHOLDER_VALUES = frozenset(
    {
        "<tag-or-commit>",
        "tag-or-commit",
        "<commit>",
        "commit",
        "<firmware-ref>",
        "firmware-ref",
        "latest",
        "head",
        "main",
        "master",
        "current",
        "local build",
        "unknown",
        "n/a",
        "na",
        "placeholder",
        "tester",
        "name",
        "todo",
        "tbd",
    }
)
GENERIC_ARTIFACT_VALUES = frozenset(
    {
        "ok",
        "passed",
        "tested ok",
        "tested ok on hardware",
        "works",
        "works as expected",
        "see attached evidence",
        "synthetic complete evidence overlay for gate testing",
    }
)
CONCRETE_EVIDENCE_RE = re.compile(
    r"(0x[0-9a-f]+|/|\\|\.log\b|\.txt\b|\.csv\b|\.png\b|\.jpg\b|\.jpeg\b|\.mp4\b|"
    r"photo|video|screenshot|log|probe|scope|oscilloscope|logic analyzer|"
    r"multimeter|serial|i2c|register|rdy|vial observation|vial screenshot|"
    r"ble trace|pairing log|uf2)",
    re.IGNORECASE,
)
EVIDENCE_UPDATE_FIELDS = (
    "status",
    "validated_at",
    "tester",
    "firmware_ref",
    "artifact_or_notes",
)
MARKDOWN_HEADING_RE = re.compile(r"^\s{0,3}#{1,6}\s+(.+?)\s*#*\s*$")
ISO_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
PLACEHOLDER_NOTE_MARKER_RE = re.compile(
    r"\b(todo|tbd|placeholder|unknown)\b",
    re.IGNORECASE,
)
COPY_AID_PLACEHOLDER_RE = re.compile(
    re.escape("<photo/log/probe/Vial path or reading>"),
    re.IGNORECASE,
)


@dataclass
class HardwareValidationSummary:
    total: int
    validated: int
    check_inventory_sha256: str
    by_status: dict[str, int]
    by_area: dict[str, dict[str, Any]]
    by_side: dict[str, dict[str, Any]]
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


def is_placeholder_value(value: Any) -> bool:
    text = str(value).strip()
    return (
        text.lower() in PLACEHOLDER_VALUES
        or (text.startswith("<") and text.endswith(">") and len(text) > 2)
    )


def validate_evidence_needles_schema(check_id: str, check: dict[str, Any]) -> list[str]:
    evidence_needles = check.get("evidence_needles")
    if not isinstance(evidence_needles, list) or not evidence_needles:
        return [f"{check_id}: evidence_needles must be a non-empty string array"]
    if not all(isinstance(needle, str) and needle.strip() for needle in evidence_needles):
        return [f"{check_id}: evidence_needles must be a non-empty string array"]
    return []


def validate_evidence_artifacts_schema(check_id: str, check: dict[str, Any]) -> list[str]:
    evidence_artifacts = check.get("evidence_artifacts")
    if not isinstance(evidence_artifacts, list) or not evidence_artifacts:
        return [f"{check_id}: evidence_artifacts must be a non-empty string array"]
    if not all(
        isinstance(artifact, str) and artifact.strip() for artifact in evidence_artifacts
    ):
        return [f"{check_id}: evidence_artifacts must be a non-empty string array"]
    return []


def artifact_note_mentions_needle(artifact_or_notes: str, needle: str) -> bool:
    pattern = re.compile(
        rf"(?<![A-Za-z0-9_]){re.escape(needle.strip())}(?![A-Za-z0-9_])",
        re.IGNORECASE,
    )
    return pattern.search(artifact_or_notes) is not None


def evidence_artifacts_text(check: dict[str, Any]) -> str:
    return ", ".join(str(artifact) for artifact in check.get("evidence_artifacts", []))


def validate_validated_evidence(check_id: str, check: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    validated_at = str(check.get("validated_at", "")).strip()
    tester = str(check.get("tester", "")).strip()
    firmware_ref = str(check.get("firmware_ref", "")).strip()
    artifact_or_notes = str(check.get("artifact_or_notes", "")).strip()

    if validated_at and not ISO_DATE_RE.fullmatch(validated_at):
        errors.append(f"{check_id}: validated_at must use YYYY-MM-DD format")
    elif validated_at:
        try:
            validation_date = datetime.date.fromisoformat(validated_at)
        except ValueError:
            errors.append(f"{check_id}: validated_at must be a real calendar date")
        else:
            if validation_date > datetime.date.today():
                errors.append(f"{check_id}: validated_at must not be in the future")

    if tester and is_placeholder_value(tester):
        errors.append(f"{check_id}: tester must identify the person or bench that ran the check")

    if firmware_ref and is_placeholder_value(firmware_ref):
        errors.append(f"{check_id}: firmware_ref must be the actual flashed tag or commit")

    if artifact_or_notes and is_placeholder_value(artifact_or_notes):
        errors.append(f"{check_id}: artifact_or_notes must describe the observed evidence")
    elif artifact_or_notes and PLACEHOLDER_NOTE_MARKER_RE.search(artifact_or_notes):
        errors.append(f"{check_id}: artifact_or_notes must not contain placeholder markers")
    elif artifact_or_notes and COPY_AID_PLACEHOLDER_RE.search(artifact_or_notes):
        errors.append(f"{check_id}: artifact_or_notes must not contain placeholder markers")
    elif artifact_or_notes and artifact_or_notes.strip().lower() in GENERIC_ARTIFACT_VALUES:
        errors.append(f"{check_id}: artifact_or_notes must describe the observed evidence")
    elif artifact_or_notes and len(artifact_or_notes) < 12:
        errors.append(
            f"{check_id}: artifact_or_notes must include a specific photo/log/probe/Vial observation note"
        )
    elif artifact_or_notes and not CONCRETE_EVIDENCE_RE.search(artifact_or_notes):
        errors.append(
            f"{check_id}: artifact_or_notes must include a concrete photo/log/probe/Vial observation note"
        )

    evidence_needles = check.get("evidence_needles", [])
    if isinstance(evidence_needles, list) and all(
        isinstance(needle, str) and needle.strip() for needle in evidence_needles
    ):
        missing_needles = [
            needle
            for needle in evidence_needles
            if not artifact_note_mentions_needle(artifact_or_notes, needle)
        ]
        if missing_needles:
            errors.append(
                f"{check_id}: artifact_or_notes must mention required observation(s): "
                + ", ".join(repr(needle) for needle in missing_needles)
            )

    evidence_artifacts = check.get("evidence_artifacts", [])
    if isinstance(evidence_artifacts, list) and all(
        isinstance(artifact, str) and artifact.strip() for artifact in evidence_artifacts
    ):
        missing_artifacts = [
            artifact
            for artifact in evidence_artifacts
            if not artifact_note_mentions_needle(artifact_or_notes, artifact)
        ]
        if missing_artifacts:
            errors.append(
                f"{check_id}: artifact_or_notes must mention required evidence artifact(s): "
                + ", ".join(repr(artifact) for artifact in missing_artifacts)
            )

    return errors


def manifest_inventory_items(manifest: dict[str, Any]) -> list[dict[str, str]]:
    checks = manifest.get("checks", [])
    if not isinstance(checks, list):
        return []
    fields = ("id", "area", "side", "requirement", "evidence", "source", "status")
    return [
        {
            **{field: str(check.get(field, "")) for field in fields},
            "evidence_needles": json.dumps(
                check.get("evidence_needles", []),
                sort_keys=True,
                separators=(",", ":"),
            ),
            "evidence_artifacts": json.dumps(
                check.get("evidence_artifacts", []),
                sort_keys=True,
                separators=(",", ":"),
            ),
        }
        for check in checks
        if isinstance(check, dict)
    ]


def manifest_inventory_digest(manifest: dict[str, Any]) -> tuple[int, str]:
    items = manifest_inventory_items(manifest)
    payload = json.dumps(items, sort_keys=True, separators=(",", ":")).encode()
    return len(items), hashlib.sha256(payload).hexdigest()


def manifest_baseline_counts(manifest: dict[str, Any]) -> dict[str, Any]:
    checks = manifest.get("checks", [])
    by_status = {status: 0 for status in sorted(VALID_STATUSES)}
    by_area: dict[str, int] = {}
    by_side: dict[str, int] = {}
    if not isinstance(checks, list):
        return {"total": 0, "by_status": by_status, "by_area": {}, "by_side": {}}

    for check in checks:
        if not isinstance(check, dict):
            continue
        status = str(check.get("status", ""))
        by_status[status] = by_status.get(status, 0) + 1
        area = str(check.get("area", ""))
        side = str(check.get("side", ""))
        by_area[area] = by_area.get(area, 0) + 1
        by_side[side] = by_side.get(side, 0) + 1

    return {
        "total": len([check for check in checks if isinstance(check, dict)]),
        "by_status": by_status,
        "by_area": dict(sorted(by_area.items())),
        "by_side": dict(sorted(by_side.items())),
    }


def table_int(value: Any, key: str, errors: list[str], label: str) -> int | None:
    if not isinstance(value, dict):
        errors.append(f"{label}: baseline section is missing")
        return None
    actual = value.get(key)
    if not isinstance(actual, int):
        errors.append(f"{label}: baseline missing integer field {key}")
        return None
    return actual


def compare_int(expected: int | None, actual: int, label: str, errors: list[str]) -> None:
    if expected is not None and expected != actual:
        errors.append(f"{label}: expected baseline {expected}, got {actual}")


def hardware_baseline_errors(baseline: dict[str, Any], manifest: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    hardware = baseline.get("hardware_validation", {})
    if not isinstance(hardware, dict):
        return ["hardware baseline must contain a [hardware_validation] table"]

    total, inventory_sha256 = manifest_inventory_digest(manifest)
    counts = manifest_baseline_counts(manifest)
    compare_int(
        table_int(hardware, "total", errors, "hardware_validation"),
        total,
        "hardware_validation.total",
        errors,
    )
    expected_sha256 = hardware.get("check_inventory_sha256")
    if not isinstance(expected_sha256, str):
        errors.append("hardware_validation: baseline missing field check_inventory_sha256")
    elif expected_sha256 != inventory_sha256:
        errors.append(
            "hardware_validation.check_inventory_sha256: "
            f"expected baseline {expected_sha256}, got {inventory_sha256}"
        )

    for key, actual_counts in [
        ("by_status", counts["by_status"]),
        ("by_area", counts["by_area"]),
        ("by_side", counts["by_side"]),
    ]:
        expected_counts = hardware.get(key)
        if not isinstance(expected_counts, dict):
            errors.append(f"hardware_validation baseline must contain [hardware_validation.{key}]")
            continue
        expected_keys = set(expected_counts)
        actual_keys = set(actual_counts)
        for missing in sorted(expected_keys - actual_keys):
            errors.append(
                f"hardware_validation.{key}.{missing}: baseline key is missing from actual manifest"
            )
        for missing in sorted(actual_keys - expected_keys):
            errors.append(
                f"hardware_validation.{key}.{missing}: actual manifest key is missing from baseline"
            )
        for item_key in sorted(expected_keys & actual_keys):
            expected = expected_counts[item_key]
            if not isinstance(expected, int):
                errors.append(f"hardware_validation.{key}.{item_key}: baseline value must be an integer")
                continue
            actual = actual_counts[item_key]
            if expected != actual:
                errors.append(
                    f"hardware_validation.{key}.{item_key}: expected baseline {expected}, got {actual}"
                )

    return errors


def merge_evidence(
    manifest: dict[str, Any],
    evidence_docs: list[dict[str, Any]],
    require_inventory_match: bool = False,
) -> tuple[dict[str, Any], list[str]]:
    merged = copy.deepcopy(manifest)
    checks = merged.get("checks", [])
    errors: list[str] = []
    if not isinstance(checks, list):
        return merged, errors
    _, expected_inventory_sha256 = manifest_inventory_digest(manifest)

    by_id: dict[str, dict[str, Any]] = {}
    for check in checks:
        if isinstance(check, dict) and isinstance(check.get("id"), str):
            by_id[check["id"]] = check

    seen_evidence: set[str] = set()
    for doc_index, evidence_doc in enumerate(evidence_docs):
        metadata = evidence_doc.get("metadata", {})
        actual_inventory_sha256 = ""
        if isinstance(metadata, dict):
            actual_inventory_sha256 = str(metadata.get(EVIDENCE_INVENTORY_FIELD, "")).strip()
        elif metadata:
            errors.append(f"evidence document #{doc_index + 1}: metadata must be a table")
        if actual_inventory_sha256:
            if actual_inventory_sha256 != expected_inventory_sha256:
                errors.append(
                    f"evidence document #{doc_index + 1}: {EVIDENCE_INVENTORY_FIELD} "
                    f"{actual_inventory_sha256!r} does not match current manifest "
                    f"{expected_inventory_sha256!r}"
                )
        elif require_inventory_match:
            errors.append(
                f"evidence document #{doc_index + 1}: missing metadata.{EVIDENCE_INVENTORY_FIELD}"
            )

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
    required_artifact_pair_sha256: str | None = None,
) -> HardwareValidationSummary:
    checks = manifest.get("checks", [])
    _, check_inventory_sha256 = manifest_inventory_digest(manifest)
    by_status = {status: 0 for status in sorted(VALID_STATUSES)}
    by_area: dict[str, dict[str, Any]] = {}
    by_side: dict[str, dict[str, Any]] = {}
    remaining: list[dict[str, str]] = []
    errors = list(initial_errors or [])
    seen_ids: set[str] = set()
    validated = 0

    if not isinstance(checks, list):
        return HardwareValidationSummary(
            0,
            0,
            check_inventory_sha256,
            by_status,
            {},
            {},
            [],
            ["checks must be an array"],
        )
    if not checks:
        errors.append("checks must contain at least one hardware validation item")

    def progress_bucket(group: dict[str, dict[str, Any]], key: str) -> dict[str, Any]:
        bucket = group.setdefault(
            key,
            {
                "total": 0,
                "validated": 0,
                "rate": None,
                "by_status": {status: 0 for status in sorted(VALID_STATUSES)},
            },
        )
        return bucket

    def append_remaining(check: dict[str, Any], status_text: str) -> None:
        remaining.append(
            {
                "id": str(check.get("id", "")),
                "area": str(check.get("area", "")),
                "side": str(check.get("side", "")),
                "status": status_text,
                "evidence": str(check.get("evidence", "")),
                "evidence_needles": ", ".join(
                    str(needle) for needle in check.get("evidence_needles", [])
                ),
                "evidence_artifacts": ", ".join(
                    str(artifact) for artifact in check.get("evidence_artifacts", [])
                ),
            }
        )

    for index, check in enumerate(checks):
        if not isinstance(check, dict):
            errors.append(f"check #{index + 1} must be a table")
            continue

        check_id = str(check.get("id", f"#{index + 1}"))
        area = str(check.get("area", ""))
        side = str(check.get("side", ""))
        area_bucket = progress_bucket(by_area, area)
        side_bucket = progress_bucket(by_side, side)
        for bucket in [area_bucket, side_bucket]:
            bucket["total"] += 1

        missing = [field for field in REQUIRED_FIELDS if not str(check.get(field, "")).strip()]
        if missing:
            errors.append(f"{check_id}: missing required field(s): {', '.join(missing)}")
        errors.extend(validate_evidence_needles_schema(check_id, check))
        errors.extend(validate_evidence_artifacts_schema(check_id, check))
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
            area_bucket["by_status"][status] += 1
            side_bucket["by_status"][status] += 1
            counts_as_validated = False
            if status in VALIDATED_STATUSES:
                validation_errors: list[str] = []
                missing_evidence = [
                    field
                    for field in VALIDATED_EVIDENCE_FIELDS
                    if not str(check.get(field, "")).strip()
                ]
                if missing_evidence:
                    validation_errors.append(
                        f"{check_id}: validated checks require evidence field(s): "
                        f"{', '.join(missing_evidence)}"
                    )
                else:
                    evidence_errors = validate_validated_evidence(check_id, check)
                    if evidence_errors:
                        validation_errors.extend(evidence_errors)
                    elif (
                        required_firmware_ref is not None
                        and str(check.get("firmware_ref", "")) != required_firmware_ref
                    ):
                        validation_errors.append(
                            f"{check_id}: validated firmware_ref "
                            f"{str(check.get('firmware_ref', ''))!r} does not match "
                            f"required {required_firmware_ref!r}"
                        )
                    elif (
                        required_artifact_pair_sha256 is not None
                        and required_artifact_pair_sha256
                        not in str(check.get("artifact_or_notes", ""))
                    ):
                        validation_errors.append(
                            f"{check_id}: artifact_or_notes must mention firmware artifact "
                            f"pair_sha256 {required_artifact_pair_sha256}"
                        )
                if validation_errors:
                    errors.extend(validation_errors)
                    append_remaining(check, "validated_invalid")
                else:
                    validated += 1
                    counts_as_validated = True
            else:
                append_remaining(check, status)
            if counts_as_validated:
                area_bucket["validated"] += 1
                side_bucket["validated"] += 1

    for group in [by_area, by_side]:
        for bucket in group.values():
            bucket["rate"] = (
                None
                if bucket["total"] == 0
                else bucket["validated"] / bucket["total"] * 100
            )

    return HardwareValidationSummary(
        total=len(checks),
        validated=validated,
        check_inventory_sha256=check_inventory_sha256,
        by_status=by_status,
        by_area=dict(sorted(by_area.items())),
        by_side=dict(sorted(by_side.items())),
        remaining=remaining,
        errors=errors,
    )


def as_json(summary: HardwareValidationSummary) -> dict[str, Any]:
    return {
        "total": summary.total,
        "validated": summary.validated,
        "rate": summary.rate,
        "check_inventory_sha256": summary.check_inventory_sha256,
        "by_status": summary.by_status,
        "by_area": summary.by_area,
        "by_side": summary.by_side,
        "remaining": summary.remaining,
        "errors": summary.errors,
        "classified": summary.classified,
    }


def markdown_escape(value: Any) -> str:
    return str(value).replace("|", "\\|").replace("\n", " ")


def evidence_needles_text(check: dict[str, Any]) -> str:
    return ", ".join(str(needle) for needle in check.get("evidence_needles", []))


def artifact_or_notes_copy_hint(check: dict[str, Any], artifact_prefix: str = "") -> str:
    required_observations = evidence_needles_text(check)
    required_artifacts = evidence_artifacts_text(check)
    parts = []
    if artifact_prefix:
        parts.append(artifact_prefix.strip().rstrip(";").strip())
    parts.append("<photo/log/probe/Vial path or reading>")
    if required_artifacts:
        parts.append(f"artifact: {required_artifacts}")
    if required_observations:
        parts.append(f"observed: {required_observations}")
    return "; ".join(parts)


def toml_string(value: Any) -> str:
    return json.dumps(str(value))


def toml_comment(value: Any) -> list[str]:
    lines = str(value).splitlines() or [""]
    return [f"# {line}" if line else "#" for line in lines]


def as_evidence_template(
    manifest: dict[str, Any], firmware_ref: str = "", artifact_pair_sha256: str = ""
) -> str:
    _, inventory_sha256 = manifest_inventory_digest(manifest)
    lines = [
        "# Hardware validation evidence overlay.",
        "# Fill this file after testing real hardware, then run:",
        "#",
        "#   python3 tools/hardware_validation.py --evidence path/to/evidence.toml --markdown",
        "#   python3 tools/hardware_validation.py --evidence path/to/evidence.toml --require-validated",
        "#   python3 tools/hardware_validation.py --evidence path/to/evidence.toml --require-firmware-ref <tag-or-commit>",
        "#",
        "# Entries are keyed by id from tools/hardware_validation_manifest.toml.",
        "# The metadata hash binds this evidence file to the current hardware validation manifest.",
        "# Change status to \"validated\" only when validated_at, tester, firmware_ref, and artifact_or_notes are filled.",
        "# If artifact_or_notes is prefilled with firmware artifact pair_sha256, keep it and append the observed evidence after it.",
        "",
        "[metadata]",
        f"{EVIDENCE_INVENTORY_FIELD} = {toml_string(inventory_sha256)}",
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
            lines.append(f"firmware_ref = {toml_string(firmware_ref)}")
            artifact_prefix = (
                f"firmware artifact pair_sha256 {artifact_pair_sha256}; "
                if artifact_pair_sha256
                else ""
            )
            lines.append(f"artifact_or_notes = {toml_string(artifact_prefix)}")
            lines.append("# Requirement:")
            lines.extend(toml_comment(check.get("requirement", "")))
            lines.append("# Required evidence:")
            lines.extend(toml_comment(check.get("evidence", "")))
            lines.append("# Evidence source:")
            lines.extend(toml_comment(check.get("source", "")))
            evidence_needles = check.get("evidence_needles", [])
            if evidence_needles:
                evidence_artifacts = check.get("evidence_artifacts", [])
                if evidence_artifacts:
                    lines.append("# Artifact/notes must include evidence artifact(s):")
                    lines.extend(
                        toml_comment(", ".join(str(artifact) for artifact in evidence_artifacts))
                    )
                lines.append("# Artifact/notes must mention:")
                lines.extend(toml_comment(", ".join(str(needle) for needle in evidence_needles)))
                lines.append("# Copy aid after this item passes on hardware:")
                lines.extend(
                    toml_comment(
                        "artifact_or_notes = "
                        + toml_string(artifact_or_notes_copy_hint(check, artifact_prefix))
                    )
                )
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
        f"Check inventory SHA256: `{markdown_escape(summary.check_inventory_sha256)}`",
        "",
        f"Status: {status_counts or 'none'}",
        "",
        "### Progress By Area",
        "",
        "| Area | Validated | Total | Rate | Status counts |",
        "| --- | --- | --- | --- | --- |",
    ]
    for area, progress in summary.by_area.items():
        lines.append(progress_markdown_row("Area", area, progress))
    lines.extend(
        [
            "",
            "### Progress By Side",
            "",
            "| Side | Validated | Total | Rate | Status counts |",
            "| --- | --- | --- | --- | --- |",
        ]
    )
    for side, progress in summary.by_side.items():
        lines.append(progress_markdown_row("Side", side, progress))
    lines.extend(
        [
            "",
            "### Checks",
            "",
            "| ID | Area | Side | Status | Requirement | Required evidence | Required artifacts | Required observations | Validated at | Tester | Firmware ref | Artifact/notes |",
            "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for check in manifest.get("checks", []):
        if not isinstance(check, dict):
            continue
        lines.append(
            "| {id} | {area} | {side} | `{status}` | {requirement} | {evidence} | {evidence_artifacts} | {evidence_needles} | {validated_at} | {tester} | {firmware_ref} | {artifact_or_notes} |".format(
                id=markdown_escape(check.get("id", "")),
                area=markdown_escape(check.get("area", "")),
                side=markdown_escape(check.get("side", "")),
                status=markdown_escape(check.get("status", "")),
                requirement=markdown_escape(check.get("requirement", "")),
                evidence=markdown_escape(check.get("evidence", "")),
                evidence_artifacts=markdown_escape(evidence_artifacts_text(check)),
                evidence_needles=markdown_escape(evidence_needles_text(check)),
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


def as_checklist(manifest: dict[str, Any]) -> str:
    lines = [
        "# LaLaPad Gen2 RMK Hardware Validation Checklist",
        "",
        "Record the flashed firmware tag or commit before testing. After each item "
        "passes, copy the check id into an evidence overlay generated with "
        "`--evidence-template` and include the required observations in "
        "`artifact_or_notes`.",
        "",
        "When using `firmware-artifacts.local.json`, keep the generated "
        "`pair_sha256` in each `artifact_or_notes` entry and append the bench "
        "observation after it.",
        "",
    ]
    checks = manifest.get("checks", [])
    current_area: str | None = None
    if isinstance(checks, list):
        for check in checks:
            if not isinstance(check, dict):
                continue
            area = str(check.get("area", ""))
            if area != current_area:
                current_area = area
                lines.extend([f"## {area or 'unknown area'}", ""])
            check_id = str(check.get("id", ""))
            side = str(check.get("side", ""))
            lines.append(f"- [ ] `{check_id}` ({side})")
            lines.append(f"  - Requirement: {check.get('requirement', '')}")
            lines.append(f"  - How to verify: {check.get('evidence', '')}")
            lines.append(f"  - Required observations: {evidence_needles_text(check)}")
            lines.append(f"  - Required artifacts: {evidence_artifacts_text(check)}")
            lines.append(f"  - Evidence source: {check.get('source', '')}")
            lines.append("  - Evidence overlay:")
            lines.append(f"    - id: {check_id}")
            lines.append("    - status: validated only after this item passes on hardware")
            lines.append("    - validated_at: YYYY-MM-DD test date")
            lines.append("    - tester: person or bench that ran the check")
            lines.append("    - firmware_ref: flashed immutable tag or commit")
            lines.append(
                "    - artifact_or_notes: concrete photo/log/probe/Vial observation "
                f"that mentions artifacts [{evidence_artifacts_text(check)}] and "
                f"observations [{evidence_needles_text(check)}]"
            )
            lines.append(
                "    - copy aid after pass: "
                f"{artifact_or_notes_copy_hint(check)}"
            )
            lines.append("")
    return "\n".join(lines)


def progress_markdown_row(label: str, value: str, progress: dict[str, Any]) -> str:
    rate = progress.get("rate")
    rate_text = "n/a" if rate is None else f"{rate:.2f}%"
    status_counts = ", ".join(
        f"`{status}`={count}"
        for status, count in progress.get("by_status", {}).items()
        if count
    )
    return (
        f"| {markdown_escape(value or f'unknown {label.lower()}')} | "
        f"{progress.get('validated', 0)} | {progress.get('total', 0)} | "
        f"{rate_text} | {status_counts or 'none'} |"
    )


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
    print(f"Hardware check inventory SHA256: {summary.check_inventory_sha256}")
    for label, group in [("area", summary.by_area), ("side", summary.by_side)]:
        if group:
            print(f"Hardware validation by {label}:")
            for key, progress in group.items():
                rate = progress.get("rate")
                rate_text = "n/a" if rate is None else f"{rate:.2f}%"
                print(
                    f"- {key or f'unknown {label}'}: "
                    f"{progress.get('validated', 0)}/{progress.get('total', 0)} = "
                    f"{rate_text}"
                )
    if summary.remaining:
        print("Hardware validation remaining:")
        for item in summary.remaining:
            print(
                f"- {item['id']} ({item['area']}/{item['side']}): "
                f"{item['status']} - {item['evidence']} "
                f"[artifacts: {item.get('evidence_artifacts', '')}; "
                f"needs: {item.get('evidence_needles', '')}]"
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
        "--hardware-baseline",
        type=Path,
        default=None,
        help="fail if the hardware validation manifest inventory drifts",
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
        "--checklist",
        action="store_true",
        help="print a human checklist for collecting real-hardware evidence",
    )
    parser.add_argument(
        "--firmware-ref-template",
        default="",
        metavar="REF",
        help="pre-fill firmware_ref in --evidence-template output",
    )
    parser.add_argument(
        "--artifact-pair-sha256-template",
        default="",
        metavar="SHA256",
        help="pre-fill the firmware artifact pair_sha256 prefix in --evidence-template artifact_or_notes",
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
        "--require-evidence-inventory",
        action="store_true",
        help="fail unless every evidence file declares the current hardware check inventory SHA256",
    )
    parser.add_argument(
        "--require-firmware-ref",
        metavar="REF",
        help="fail if any validated hardware evidence was captured against a different firmware tag or commit",
    )
    args = parser.parse_args()

    output_modes = sum(
        bool(mode)
        for mode in [args.json, args.markdown, args.evidence_template, args.checklist]
    )
    if output_modes > 1:
        parser.error(
            "--json, --markdown, --evidence-template, and --checklist are mutually exclusive"
        )
    if args.firmware_ref_template and not args.evidence_template:
        parser.error("--firmware-ref-template can only be used with --evidence-template")
    if args.artifact_pair_sha256_template and not args.evidence_template:
        parser.error(
            "--artifact-pair-sha256-template can only be used with --evidence-template"
        )
    if args.artifact_pair_sha256_template and not re.fullmatch(
        r"[0-9a-f]{64}", args.artifact_pair_sha256_template
    ):
        parser.error("--artifact-pair-sha256-template must be a SHA256 hex string")

    manifest_doc = load_toml(args.manifest)
    baseline_failures: list[str] = []
    if args.hardware_baseline is not None:
        try:
            baseline = load_toml(args.hardware_baseline)
        except OSError as e:
            baseline_failures = [f"failed to read {args.hardware_baseline}: {e}"]
        else:
            baseline_failures = hardware_baseline_errors(baseline, manifest_doc)

    manifest, evidence_errors = merge_evidence(
        manifest_doc,
        [load_toml(path) for path in args.evidence],
        args.require_evidence_inventory,
    )
    summary = summarize(
        manifest,
        evidence_errors + baseline_failures,
        Path("."),
        args.require_firmware_ref,
    )
    if args.json:
        print(json.dumps(as_json(summary), indent=2, sort_keys=True))
    elif args.markdown:
        print(as_markdown(manifest, summary), end="")
    elif args.evidence_template:
        print(
            as_evidence_template(
                manifest,
                args.firmware_ref_template,
                args.artifact_pair_sha256_template,
            ),
            end="",
        )
    elif args.checklist:
        print(as_checklist(manifest), end="")
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
