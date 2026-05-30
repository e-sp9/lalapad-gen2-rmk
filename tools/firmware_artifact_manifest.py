#!/usr/bin/env python3
"""Generate a reproducible manifest for firmware artifacts used on hardware."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import zipfile
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))

import firmware_artifact_specs
import hardware_validation


ARTIFACTS = firmware_artifact_specs.ARTIFACTS


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def git_ref(root: Path) -> str:
    for command in [
        ["git", "describe", "--tags", "--exact-match"],
        ["git", "rev-parse", "--short=12", "HEAD"],
    ]:
        try:
            output = subprocess.check_output(
                command,
                cwd=root,
                stderr=subprocess.DEVNULL,
                text=True,
            ).strip()
        except (OSError, subprocess.CalledProcessError):
            continue
        if output:
            return output
    return ""


def dfu_manifest(path: Path) -> dict[str, Any]:
    try:
        with zipfile.ZipFile(path) as archive:
            try:
                raw = archive.read("manifest.json")
            except KeyError:
                return {"valid": False, "error": "manifest.json missing"}
            try:
                manifest = json.loads(raw.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError) as e:
                return {"valid": False, "error": f"manifest.json is invalid JSON: {e}"}
    except zipfile.BadZipFile as e:
        return {"valid": False, "error": f"invalid zip file: {e}"}
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


def artifact_entry(
    root: Path, spec: firmware_artifact_specs.ArtifactSpec
) -> dict[str, Any] | None:
    path = root / spec.path
    if not path.exists():
        return None
    entry: dict[str, Any] = {
        "path": spec.path,
        "role": spec.role,
        "side": spec.side,
        "kind": spec.kind,
        "size": path.stat().st_size,
        "sha256": sha256_file(path),
    }
    if spec.kind == "adafruit-nrf52-dfu-zip":
        entry["dfu_manifest"] = dfu_manifest(path)
    return entry


def build_manifest(
    root: Path,
    firmware_ref: str,
    require_uf2: bool,
    require_reset_uf2: bool,
    require_dfu: bool,
) -> tuple[dict[str, Any], list[str]]:
    artifacts: list[dict[str, Any]] = []
    errors: list[str] = []
    required_groups = set()
    if require_uf2:
        required_groups.add("uf2")
    if require_reset_uf2:
        required_groups.add("reset_uf2")
    if require_dfu:
        required_groups.add("dfu")

    for spec in ARTIFACTS:
        entry = artifact_entry(root, spec)
        if entry is None:
            if spec.required_group in required_groups:
                errors.append(f"missing required artifact: {spec.path}")
            continue
        if spec.kind == firmware_artifact_specs.DFU_ARTIFACT_KIND:
            dfu = entry.get("dfu_manifest", {})
            if not isinstance(dfu, dict) or not dfu.get("valid"):
                reason = ""
                if isinstance(dfu, dict):
                    reason = str(dfu.get("error", "")).strip()
                suffix = f": {reason}" if reason else ""
                errors.append(f"invalid DFU artifact {spec.path}{suffix}")
        artifacts.append(entry)

    pair_digest_payload = json.dumps(
        [
            {
                "path": artifact["path"],
                "size": artifact["size"],
                "sha256": artifact["sha256"],
            }
            for artifact in artifacts
        ],
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    manifest = {
        "firmware_ref": firmware_ref,
        "artifact_count": len(artifacts),
        "pair_sha256": hashlib.sha256(pair_digest_payload).hexdigest(),
        "artifacts": artifacts,
    }
    return manifest, errors


def as_markdown(manifest: dict[str, Any]) -> str:
    lines = [
        "## Firmware Artifact Manifest",
        "",
        f"- Firmware ref: `{manifest['firmware_ref']}`",
        f"- Artifact count: {manifest['artifact_count']}",
        f"- Pair SHA256: `{manifest['pair_sha256']}`",
        "",
        "| Role | Side | Kind | Path | Size | SHA256 |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for artifact in manifest["artifacts"]:
        lines.append(
            "| {role} | {side} | {kind} | `{path}` | {size} | `{sha256}` |".format(
                role=artifact["role"],
                side=artifact["side"],
                kind=artifact["kind"],
                path=artifact["path"],
                size=artifact["size"],
                sha256=artifact["sha256"],
            )
        )
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--firmware-ref", default=None)
    parser.add_argument("--require-uf2", action="store_true")
    parser.add_argument("--require-reset-uf2", action="store_true")
    parser.add_argument("--require-dfu", action="store_true")
    parser.add_argument("--markdown", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args()

    root = args.root.resolve()
    firmware_ref = args.firmware_ref if args.firmware_ref is not None else git_ref(root)
    if not firmware_ref.strip() or hardware_validation.is_mutable_firmware_ref(firmware_ref):
        if args.output is not None:
            output = args.output
            if not output.is_absolute():
                output = root / output
            output.unlink(missing_ok=True)
        parser.error("--firmware-ref must be an immutable flashed tag or commit")
    manifest, errors = build_manifest(
        root,
        firmware_ref,
        args.require_uf2,
        args.require_reset_uf2,
        args.require_dfu,
    )
    if args.markdown:
        rendered = as_markdown(manifest)
    else:
        rendered = json.dumps(manifest, indent=2, sort_keys=True) + "\n"

    if errors and args.output is not None:
        output = args.output
        if not output.is_absolute():
            output = root / output
        output.unlink(missing_ok=True)
    elif args.output is not None:
        output = args.output
        if not output.is_absolute():
            output = root / output
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(rendered, encoding="utf-8")
    else:
        print(rendered, end="")

    if errors:
        print("firmware artifact manifest errors:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        raise SystemExit(1)

if __name__ == "__main__":
    main()
