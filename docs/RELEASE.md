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
- `firmware/reset/lalapad-gen2-rmk-reset-central.uf2`
- `firmware/reset/lalapad-gen2-rmk-reset-peripheral.uf2`

Generate a local artifact hash manifest after the UF2 files are built:

```sh
python3 tools/firmware_artifact_manifest.py --require-uf2 --require-reset-uf2 > firmware-artifacts.local.json
cargo make firmware-artifact-manifest-current
cargo make hardware-validation-session-current
```

Generated firmware files are ignored by git and should not be committed.

## Final Migration Validation

The normal CI gate proves that the source-backed RMK migration is complete and
that every hardware-only gap is classified. Before claiming a fully validated
release, collect real-device evidence with the exact firmware pair that will be
announced, then run:

```sh
HARDWARE_EVIDENCE=path/to/evidence.toml FIRMWARE_REF=tag-or-commit FIRMWARE_ARTIFACT_MANIFEST=firmware-artifacts.local.json cargo make migration-status-final
```

This wraps the same final migration status gate after running the RMK
ZMK-derived runtime scenario suite. It resolves `ZMK_KEYMAP` first when it is
set and otherwise uses the standard upstream checkout path. The final gate also
requires that resolved ZMK source checkout to be a clean Git repository at the
manifest-pinned `metadata.source_commit`.

The command must report `Full validation: pass`. If it fails, the release may
still be source-complete, but it is not yet fully validated against real
hardware for that firmware reference. When `--firmware-artifact-manifest` is
present, the final gate also requires each validated hardware evidence note to
mention the artifact manifest `pair_sha256`, so the observation is bound to the
exact normal and storage-clear UF2 files that were flashed. It also re-reads the local
artifact files listed by the manifest under `--artifact-root` (the current
directory by default) and rejects stale manifests whose size or SHA256 no
longer matches the current files. The final gate also rejects evidence files
that are missing the generated `metadata.hardware_check_inventory_sha256`, or
whose hash no longer matches the current hardware validation manifest. Each
validated note must also mention the per-check evidence artifact types listed by
`evidence_artifacts`, such as `video`, `scope`, `Vial screenshot`, or
`key-event log`, and each validated entry must list at least one existing file
in `artifact_paths`. Relative evidence artifact paths are resolved under
`EVIDENCE_ARTIFACT_ROOT` when set, or from the current directory otherwise.
The retained file types must match the required artifact types; a video-only
check cannot pass final validation with only a text log attached. Every
retained file must also be named by path or basename in `artifact_or_notes`,
and separate required artifact types must have separate retained files; a
duplicated resolved path is rejected instead of counted twice.
Use the generated `hardware-evidence/<check-id>-<artifact-type>.<ext>` path
suggestions in the evidence template/checklist unless the bench uses an
equivalent retained path that is also named in `artifact_or_notes`; the
generated suggestions assume the default `EVIDENCE_ARTIFACT_ROOT=.`. The
generated observation placeholder must be replaced with real bench output
before an evidence entry can count as validated.
Simulated, synthetic, mock, or host-only output is not valid hardware evidence.
`cargo make hardware-validation-session-current` pre-fills
that hash in the local evidence overlay generated for a clean current-ref bench
session and prints the manifest's exact firmware artifact paths, roles, sides,
sizes, and SHA256 hashes in the local checklist. Direct
`tools/migration_status.py --require-hardware-validated` use also requires
`--firmware-artifact-manifest`; the final validation gate cannot pass from an
evidence file alone. `cargo make migration-status-final-current` regenerates
the same artifact manifest path that the final gate will read before checking
evidence, so a complete current-ref validation claim is tied to the UF2 files
built from that clean commit.

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
- `cargo make migration-status-final` passes with the release evidence file, matching `FIRMWARE_REF`, and the manifest-pinned clean ZMK source commit, if the announcement claims complete hardware validation.
- The hardware evidence file keeps the generated `metadata.hardware_check_inventory_sha256` for the current validation manifest.
- The hardware evidence notes reference the artifact manifest SHA256 values for the flashed central/peripheral files.
- The hardware evidence notes include every per-check artifact type listed by `evidence_artifacts`.
- The hardware evidence entries include `artifact_paths` pointing at retained photos, videos, logs, screenshots, or scope traces under the chosen `EVIDENCE_ARTIFACT_ROOT`.
- GitHub Release contains central/peripheral UF2 and DFU zip assets.
- GitHub Release contains `lalapad-gen2-rmk-artifacts.json`.
- Web flasher loads the latest release metadata.
- Both halves flash successfully.
- BLE pairing works after deleting old host pairing.
- Matrix keys, 5-way switches, combos, tri-layer, Vial, and BLE profile controls work.
- Right and left IQS9151 trackpads work independently.
- Cursor, scroll, pinch, tap, tap-drag, and three-finger gestures are checked.
- RGB LED polarity and charge LED behavior are checked on real hardware.
- Known limitations in `README.md` and `docs/PORTING.md` still match reality.
