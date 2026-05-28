#!/usr/bin/env python3
"""Check that RMK storage cannot overlap the application firmware."""

from __future__ import annotations

import argparse
import re
import struct
import sys
import tomllib
from pathlib import Path


FLASH_CAPACITY = 0x100000
NRF52840_ERASE_SIZE = 0x1000
BOOTLOADER_RESERVED_START = 0xF0000
UF2_FILES = (
    Path("firmware/normal/lalapad-gen2-rmk-central.uf2"),
    Path("firmware/normal/lalapad-gen2-rmk-peripheral.uf2"),
)


def fail(message: str) -> None:
    print(f"flash-layout error: {message}", file=sys.stderr)
    raise SystemExit(1)


def parse_size(value: str) -> int:
    value = value.strip()
    match = re.fullmatch(r"(0x[0-9a-fA-F]+|\d+)\s*([KkMm]?)", value)
    if not match:
        fail(f"cannot parse memory size: {value!r}")

    number = int(match.group(1), 0)
    suffix = match.group(2).lower()
    if suffix == "k":
        return number * 1024
    if suffix == "m":
        return number * 1024 * 1024
    return number


def read_flash_region(memory_x: Path) -> tuple[int, int]:
    text = memory_x.read_text(encoding="utf-8")
    match = re.search(
        r"FLASH\s*:\s*ORIGIN\s*=\s*([^,]+),\s*LENGTH\s*=\s*([^\n]+)",
        text,
    )
    if not match:
        fail(f"FLASH region not found in {memory_x}")

    origin = int(match.group(1).strip(), 0)
    length = parse_size(match.group(2).strip())
    return origin, origin + length


def read_storage_region(keyboard_toml: Path) -> tuple[int, int]:
    with keyboard_toml.open("rb") as f:
        data = tomllib.load(f)

    storage = data.get("storage")
    if not storage:
        fail("[storage] must be explicit for XIAO nRF52840 builds")

    start_addr = int(storage.get("start_addr", 0))
    if start_addr == 0:
        fail("[storage].start_addr must be explicit; RMK defaults can overlap bootloader or firmware")

    num_sectors = int(storage.get("num_sectors", 2))
    if num_sectors < 2:
        fail("[storage].num_sectors must be at least 2")

    if start_addr % NRF52840_ERASE_SIZE != 0:
        fail(f"[storage].start_addr {start_addr:#x} is not sector aligned")

    storage_end = start_addr + num_sectors * NRF52840_ERASE_SIZE
    if storage_end > BOOTLOADER_RESERVED_START:
        fail(
            f"storage {start_addr:#x}..{storage_end:#x} enters reserved bootloader guard "
            f"at {BOOTLOADER_RESERVED_START:#x}"
        )

    if storage_end > FLASH_CAPACITY:
        fail(f"storage {start_addr:#x}..{storage_end:#x} exceeds flash capacity")

    return start_addr, storage_end


def read_uf2_region(path: Path) -> tuple[int, int]:
    lo: int | None = None
    hi: int | None = None

    with path.open("rb") as f:
        block_index = 0
        while block := f.read(512):
            block_index += 1
            if len(block) != 512:
                fail(f"{path} has a short UF2 block at index {block_index}")

            magic0, magic1, _flags, addr, payload_size, *_ = struct.unpack_from("<IIIIIIII", block, 0)
            if magic0 != 0x0A324655 or magic1 != 0x9E5D5157:
                fail(f"{path} has invalid UF2 magic at block {block_index}")

            end = addr + payload_size
            lo = addr if lo is None else min(lo, addr)
            hi = end if hi is None else max(hi, end)

    if lo is None or hi is None:
        fail(f"{path} is empty")

    return lo, hi


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config-only", action="store_true", help="skip generated UF2 checks")
    parser.add_argument("--require-uf2", action="store_true", help="fail if expected UF2 files are missing")
    args = parser.parse_args()

    app_start, app_end = read_flash_region(Path("memory.x"))
    storage_start, storage_end = read_storage_region(Path("keyboard.toml"))

    if app_end > storage_start:
        fail(f"memory.x FLASH {app_start:#x}..{app_end:#x} overlaps storage at {storage_start:#x}")

    print(f"memory.x application region: {app_start:#x}..{app_end:#x}")
    print(f"RMK storage region:         {storage_start:#x}..{storage_end:#x}")

    if args.config_only:
        return

    for uf2 in UF2_FILES:
        if not uf2.exists():
            if args.require_uf2:
                fail(f"missing generated UF2: {uf2}")
            continue

        image_start, image_end = read_uf2_region(uf2)
        if image_end > storage_start:
            fail(f"{uf2} writes {image_start:#x}..{image_end:#x}, overlapping storage at {storage_start:#x}")
        print(f"{uf2}: {image_start:#x}..{image_end:#x}")


if __name__ == "__main__":
    main()
