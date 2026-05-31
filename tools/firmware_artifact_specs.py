#!/usr/bin/env python3
"""Shared firmware artifact inventory used by release and validation tools."""

from __future__ import annotations

import json
import struct
import zipfile
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class ArtifactSpec:
    path: str
    role: str
    side: str
    kind: str
    required_group: str | None = None


ARTIFACTS = (
    ArtifactSpec(
        "firmware/normal/lalapad-gen2-rmk-central.uf2",
        "central",
        "right",
        "uf2",
        "uf2",
    ),
    ArtifactSpec(
        "firmware/normal/lalapad-gen2-rmk-peripheral.uf2",
        "peripheral",
        "left",
        "uf2",
        "uf2",
    ),
    ArtifactSpec(
        "firmware/hex/lalapad-gen2-rmk-central.hex",
        "central",
        "right",
        "ihex",
    ),
    ArtifactSpec(
        "firmware/hex/lalapad-gen2-rmk-peripheral.hex",
        "peripheral",
        "left",
        "ihex",
    ),
    ArtifactSpec(
        "firmware/reset/lalapad-gen2-rmk-reset-central.uf2",
        "reset-central",
        "right",
        "reset-uf2",
        "reset_uf2",
    ),
    ArtifactSpec(
        "firmware/reset/lalapad-gen2-rmk-reset-peripheral.uf2",
        "reset-peripheral",
        "left",
        "reset-uf2",
        "reset_uf2",
    ),
    ArtifactSpec(
        "firmware/lalapad-gen2-rmk-central-dfu.zip",
        "central",
        "right",
        "adafruit-nrf52-dfu-zip",
        "dfu",
    ),
    ArtifactSpec(
        "firmware/lalapad-gen2-rmk-peripheral-dfu.zip",
        "peripheral",
        "left",
        "adafruit-nrf52-dfu-zip",
        "dfu",
    ),
)


EXPECTED_METADATA_BY_PATH = {
    spec.path: {
        "role": spec.role,
        "side": spec.side,
        "kind": spec.kind,
    }
    for spec in ARTIFACTS
}

KNOWN_ARTIFACT_PATHS = frozenset(spec.path for spec in ARTIFACTS)

DFU_ARTIFACT_KIND = "adafruit-nrf52-dfu-zip"

UF2_MAGIC_0 = 0x0A324655
UF2_MAGIC_1 = 0x9E5D5157
UF2_MAGIC_END = 0x0AB16F30
UF2_BLOCK_SIZE = 512
UF2_MAX_PAYLOAD_SIZE = 476
IHEX_RECORD_TYPES = frozenset({0x00, 0x01, 0x02, 0x03, 0x04, 0x05})

HARDWARE_VALIDATION_REQUIRED_GROUPS = frozenset({"uf2", "reset_uf2"})

HARDWARE_VALIDATION_REQUIRED_ARTIFACTS = tuple(
    spec for spec in ARTIFACTS if spec.required_group in HARDWARE_VALIDATION_REQUIRED_GROUPS
)


def artifact_file_errors(path: Path, spec: ArtifactSpec) -> list[str]:
    if spec.kind in {"uf2", "reset-uf2"}:
        return uf2_file_errors(path)
    if spec.kind == "ihex":
        return ihex_file_errors(path)
    return []


def dfu_manifest(path: Path) -> dict[str, object]:
    try:
        with zipfile.ZipFile(path) as archive:
            try:
                raw = archive.read("manifest.json")
            except KeyError:
                return {"valid": False, "error": "manifest.json missing"}
            try:
                manifest = json.loads(raw.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError) as exc:
                return {"valid": False, "error": f"manifest.json is invalid JSON: {exc}"}
    except zipfile.BadZipFile as exc:
        return {"valid": False, "error": f"invalid zip file: {exc}"}
    if not isinstance(manifest, dict):
        return {"valid": False, "error": "manifest.json root must be an object"}
    app_root = manifest.get("manifest", {})
    app = app_root.get("application", {}) if isinstance(app_root, dict) else {}
    if not isinstance(app, dict):
        return {"valid": False, "error": "manifest.application must be an object"}
    application = {
        key: app.get(key)
        for key in ["bin_file", "dat_file", "init_packet_data", "firmware_size"]
        if key in app
    }
    if not all(
        isinstance(app.get(key), str) and app.get(key).strip()
        for key in ["bin_file", "dat_file"]
    ):
        return {
            "valid": False,
            "error": "manifest.application missing bin_file or dat_file",
            "application": application,
        }
    return {
        "valid": True,
        "application": application,
    }


def uf2_file_errors(path: Path) -> list[str]:
    try:
        contents = path.read_bytes()
    except OSError as exc:
        return [f"{path} is not readable: {exc}"]
    if not contents:
        return [f"{path} is empty"]
    if len(contents) % UF2_BLOCK_SIZE != 0:
        return [f"{path} size is not a whole number of UF2 blocks"]
    actual_blocks = len(contents) // UF2_BLOCK_SIZE
    declared_blocks: int | None = None
    seen_block_numbers: set[int] = set()
    for index in range(0, len(contents), UF2_BLOCK_SIZE):
        block = contents[index : index + UF2_BLOCK_SIZE]
        magic0, magic1, _flags, _target_addr, payload_size, block_number, num_blocks = (
            struct.unpack_from("<IIIIIII", block, 0)
        )
        (magic_end,) = struct.unpack_from("<I", block, 508)
        display_block_number = index // UF2_BLOCK_SIZE + 1
        if magic0 != UF2_MAGIC_0 or magic1 != UF2_MAGIC_1 or magic_end != UF2_MAGIC_END:
            return [f"{path} has invalid UF2 magic at block {display_block_number}"]
        if payload_size == 0 or payload_size > UF2_MAX_PAYLOAD_SIZE:
            return [
                f"{path} has invalid UF2 payload size {payload_size} "
                f"at block {display_block_number}"
            ]
        if num_blocks != actual_blocks:
            return [
                f"{path} declares {num_blocks} UF2 blocks but contains {actual_blocks}"
            ]
        if block_number >= num_blocks:
            return [
                f"{path} has UF2 block number {block_number} outside declared range "
                f"0..{num_blocks - 1}"
            ]
        if declared_blocks is None:
            declared_blocks = num_blocks
        elif num_blocks != declared_blocks:
            return [
                f"{path} has inconsistent UF2 numBlocks at block {display_block_number}"
            ]
        if block_number in seen_block_numbers:
            return [f"{path} repeats UF2 block number {block_number}"]
        seen_block_numbers.add(block_number)
    if seen_block_numbers != set(range(actual_blocks)):
        return [f"{path} does not contain a complete UF2 block-number sequence"]
    return []


def ihex_file_errors(path: Path) -> list[str]:
    try:
        text = path.read_text(encoding="ascii")
    except UnicodeDecodeError as exc:
        return [f"{path} is not ASCII Intel HEX: {exc}"]
    except OSError as exc:
        return [f"{path} is not readable: {exc}"]
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    if not lines:
        return [f"{path} is empty"]
    eof_seen = False
    for line_number, line in enumerate(lines, start=1):
        if not line.startswith(":"):
            return [
                f"{path} contains a non-Intel-HEX record at line {line_number}: {line!r}"
            ]
        payload = line[1:]
        if len(payload) < 10 or len(payload) % 2 != 0:
            return [f"{path} has malformed Intel HEX length at line {line_number}"]
        try:
            record = bytes.fromhex(payload)
        except ValueError as exc:
            return [f"{path} has non-hex Intel HEX data at line {line_number}: {exc}"]
        byte_count = record[0]
        address = int.from_bytes(record[1:3], "big")
        record_type = record[3]
        data = record[4:-1]
        if len(data) != byte_count:
            return [
                f"{path} Intel HEX byte count {byte_count} does not match "
                f"{len(data)} data bytes at line {line_number}"
            ]
        if record_type not in IHEX_RECORD_TYPES:
            return [
                f"{path} has unsupported Intel HEX record type {record_type} "
                f"at line {line_number}"
            ]
        if sum(record) & 0xFF:
            return [f"{path} has invalid Intel HEX checksum at line {line_number}"]
        if eof_seen:
            return [f"{path} has Intel HEX records after EOF at line {line_number}"]
        if record_type == 0x01:
            if byte_count != 0 or address != 0 or data:
                return [f"{path} has malformed Intel HEX EOF record at line {line_number}"]
            eof_seen = True
        elif record_type in {0x02, 0x03, 0x04, 0x05}:
            if address != 0:
                return [
                    f"{path} Intel HEX record type {record_type} must use address 0000 "
                    f"at line {line_number}"
                ]
            expected_byte_count = 2 if record_type in {0x02, 0x04} else 4
            if byte_count != expected_byte_count:
                return [
                    f"{path} Intel HEX record type {record_type} must contain "
                    f"{expected_byte_count} data bytes at line {line_number}"
                ]
    if not eof_seen:
        return [f"{path} is missing Intel HEX EOF record"]
    return []
