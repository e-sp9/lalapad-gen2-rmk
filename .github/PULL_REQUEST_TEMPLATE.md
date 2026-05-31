## Summary

- 

## Type

- [ ] Firmware behavior
- [ ] Keymap / Vial
- [ ] Trackpad tuning
- [ ] RGB / battery / charge behavior
- [ ] Build / release / flasher
- [ ] Documentation only

## Validation

- [ ] `python3 -m json.tool vial.json >/tmp/lalapad-vial-check.json`
- [ ] `python3 -c 'import tomllib; [tomllib.load(open(path, "rb")) for path in ("keyboard.toml", "Cargo.toml", "Makefile.toml", "tools/porting_coverage_manifest.toml", "tools/porting_coverage_baseline.toml", "tools/hardware_validation_manifest.toml", "tools/hardware_validation_baseline.toml", "tools/hardware_validation_evidence.example.toml")]; print("toml ok")'`
- [ ] `rmkit get-chip --keyboard-toml-path keyboard.toml`
- [ ] `rmkit get-project-name --keyboard-toml-path keyboard.toml`
- [ ] `cargo make porting-coverage` with the manifest-pinned ZMK source commit
- [ ] `cargo make migration-status` with the manifest-pinned ZMK source commit
- [ ] `cargo make migration-status-release-ready` with the manifest-pinned clean ZMK source commit
- [ ] `cargo make migration-status-report` with the manifest-pinned ZMK source commit
- [ ] `HARDWARE_EVIDENCE=path/to/evidence.toml FIRMWARE_REF=tag-or-commit cargo make migration-status-report`, if reviewing partial hardware evidence
- [ ] `HARDWARE_EVIDENCE=path/to/evidence.toml FIRMWARE_REF=tag-or-commit FIRMWARE_ARTIFACT_MANIFEST=firmware-artifacts.local.json cargo make migration-status-final`, if claiming complete migration validation for a release with the manifest-pinned clean ZMK source commit
- [ ] `HARDWARE_EVIDENCE=hardware-validation-evidence.local.toml cargo make migration-status-final-current`, if claiming complete migration validation for the current clean commit
- [ ] Hardware evidence keeps the generated `metadata.hardware_check_inventory_sha256`, if claiming complete hardware validation
- [ ] Hardware evidence records `artifact_path_sha256` for every retained `artifact_paths` file, if claiming complete hardware validation
- [ ] `python3 tools/hardware_validation.py --hardware-baseline tools/hardware_validation_baseline.toml --require-classified`
- [ ] `python3 tools/hardware_validation.py --markdown`
- [ ] `python3 tools/hardware_validation.py --checklist`
- [ ] `python3 tools/hardware_validation.py --evidence-template`
- [ ] `cargo make hardware-validation-evidence-template-current`, if preparing hardware evidence from the current clean commit
- [ ] `cargo make hardware-validation-session-current`, if preparing a current clean commit for a hardware bench session
- [ ] `python3 tools/hardware_validation.py --evidence-template --firmware-ref-template <tag-or-commit>`, if preparing hardware evidence for a release
- [ ] `python3 tools/hardware_validation.py --evidence-template --artifact-pair-sha256-template <sha256>`, if preparing artifact-bound hardware evidence for a release
- [ ] `python3 tools/hardware_validation.py --evidence path/to/evidence.toml --markdown`, if hardware evidence changed
- [ ] `python3 tools/hardware_validation.py --evidence path/to/evidence.toml --require-validated --require-firmware-ref <tag-or-commit>`, if claiming complete hardware validation for a release
- [ ] `cargo make rmk-zmk-scenario-tests`, if Space/Enter/system-tri-layer behavior or RMK behavior profiles changed
- [ ] `cargo make rmk-behavior-tests`
- [ ] `cargo check --release --bin central`
- [ ] `cargo check --release --bin peripheral`
- [ ] `cargo build --release`
- [ ] `cargo make uf2 --release`, if release artifacts or flashing changed
- [ ] `cargo make reset-uf2 --release`, if release artifacts, flashing, or storage reset behavior changed
- [ ] `python3 tools/firmware_artifact_manifest.py --require-uf2 --require-reset-uf2 > firmware-artifacts.local.json`, if release artifacts or flashed files changed
- [ ] `cargo make firmware-artifact-manifest-current`, if release artifacts or flashed files changed from a clean local commit

## Hardware Notes

Tested on:

- [ ] Right central
- [ ] Left peripheral
- [ ] BLE
- [ ] USB
- [ ] Web flasher

Notes:

- 

## Compatibility Notes

- [ ] Split orientation remains right central / left peripheral.
- [ ] Generated firmware artifacts are not committed.
- [ ] `docs/PORTING.md` is updated for behavior differences.
