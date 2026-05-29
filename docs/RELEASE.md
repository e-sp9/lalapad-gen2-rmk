# Release And Flashing Guide

This project builds two firmware artifacts:

- `lalapad-gen2-rmk-central` for the right half
- `lalapad-gen2-rmk-peripheral` for the left half

The split orientation follows the upstream ZMK firmware: right is central, left
is peripheral.

## Release Flow

Firmware changes pushed to `main` trigger the auto-tag workflow. The next `v*`
tag triggers the firmware workflow, which builds:

- UF2 files for manual bootloader flashing
- Intel HEX files
- Adafruit nRF52 serial DFU zip packages for the web flasher

The generated release assets are attached to the GitHub Release. The Pages
workflow bundles the latest DFU zip packages into the web flasher so browsers
can load them from the same origin.

## Manual Build

```sh
cargo make uf2 --release
```

Outputs:

- `firmware/normal/lalapad-gen2-rmk-central.uf2`
- `firmware/normal/lalapad-gen2-rmk-peripheral.uf2`

Generated firmware files are ignored by git and should not be committed.

## Final Migration Validation

The normal CI gate proves that the source-backed RMK migration is complete and
that every hardware-only gap is classified. Before claiming a fully validated
release, collect real-device evidence with the exact firmware pair that will be
announced, then run:

```sh
HARDWARE_EVIDENCE=path/to/evidence.toml FIRMWARE_REF=tag-or-commit cargo make migration-status-final
```

This wraps:

```sh
python3 tools/migration_status.py --evidence path/to/evidence.toml \
  --coverage-baseline tools/porting_coverage_baseline.toml \
  --hardware-baseline tools/hardware_validation_baseline.toml \
  --require-software-complete \
  --require-hardware-classified \
  --require-hardware-validated \
  --require-firmware-ref <tag-or-commit>
```

The command must report `Full validation: pass`. If it fails, the release may
still be source-complete, but it is not yet fully validated against real
hardware for that firmware reference.

## Flashing With UF2

1. Put the left half into the XIAO BLE bootloader by double-tapping reset.
2. Copy `lalapad-gen2-rmk-peripheral.uf2` to the mounted bootloader drive.
3. Put the right half into the XIAO BLE bootloader.
4. Copy `lalapad-gen2-rmk-central.uf2` to the mounted bootloader drive.
5. Reconnect the right half first, then the left half.
6. Remove the old BLE pairing on the host and pair again.

Removing the old BLE pairing is important after HID report map changes. Hosts
can cache the old report descriptor and keep using it until the device is
paired again.

## Flashing With The Web Flasher

Use Chrome, Edge, or Opera. The browser needs WebHID and WebSerial support.

- Right central: the flasher can request DFU mode over WebHID.
- Left peripheral: enter DFU manually by double-tapping reset.

Use matching left/right firmware from the same release.

## Pre-Announcement Checklist

Before announcing a release to the community:

- CI firmware workflow is green for the release tag.
- `cargo make migration-status-final` passes with the release evidence file and matching `FIRMWARE_REF`, if the announcement claims complete hardware validation.
- GitHub Release contains central/peripheral UF2 and DFU zip assets.
- Web flasher loads the latest release metadata.
- Both halves flash successfully.
- BLE pairing works after deleting old host pairing.
- Matrix keys, 5-way switches, combos, tri-layer, Vial, and BLE profile controls work.
- Right and left IQS9151 trackpads work independently.
- Cursor, scroll, pinch, tap, tap-drag, and three-finger gestures are checked.
- RGB LED polarity and charge LED behavior are checked on real hardware.
- Known limitations in `README.md` and `docs/PORTING.md` still match reality.
