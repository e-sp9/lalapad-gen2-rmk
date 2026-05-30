#!/usr/bin/env python3
"""Shared firmware artifact inventory used by release and validation tools."""

from __future__ import annotations

from dataclasses import dataclass


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
