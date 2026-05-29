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
- [ ] `python3 tools/porting_coverage.py --coverage-baseline tools/porting_coverage_baseline.toml --require-zmk-source --require-porting-complete`
- [ ] `python3 tools/migration_status.py --coverage-baseline tools/porting_coverage_baseline.toml --hardware-baseline tools/hardware_validation_baseline.toml --require-zmk-source --require-software-complete --require-hardware-classified`
- [ ] `cargo make migration-status-report`
- [ ] `HARDWARE_EVIDENCE=path/to/evidence.toml FIRMWARE_REF=tag-or-commit cargo make migration-status-report`, if reviewing partial hardware evidence
- [ ] `python3 tools/migration_status.py --coverage-baseline tools/porting_coverage_baseline.toml --hardware-baseline tools/hardware_validation_baseline.toml --zmk-keymap zmk-config-LalaPadGen2/config/lalapadgen2.keymap --evidence path/to/evidence.toml --require-zmk-source --require-software-complete --require-hardware-classified --require-hardware-validated --require-firmware-ref <tag-or-commit>`, if claiming complete migration validation for a release
- [ ] `HARDWARE_EVIDENCE=path/to/evidence.toml FIRMWARE_REF=tag-or-commit cargo make migration-status-final`, if claiming complete migration validation for a release
- [ ] `HARDWARE_EVIDENCE=hardware-validation-evidence.local.toml cargo make migration-status-final-current`, if claiming complete migration validation for the current clean commit
- [ ] `python3 tools/hardware_validation.py --hardware-baseline tools/hardware_validation_baseline.toml --require-classified`
- [ ] `python3 tools/hardware_validation.py --markdown`
- [ ] `python3 tools/hardware_validation.py --checklist`
- [ ] `python3 tools/hardware_validation.py --evidence-template`
- [ ] `cargo make hardware-validation-evidence-template-current`, if preparing hardware evidence from the current clean commit
- [ ] `python3 tools/hardware_validation.py --evidence-template --firmware-ref-template <tag-or-commit>`, if preparing hardware evidence for a release
- [ ] `python3 tools/hardware_validation.py --evidence path/to/evidence.toml --markdown`, if hardware evidence changed
- [ ] `python3 tools/hardware_validation.py --evidence path/to/evidence.toml --require-validated --require-firmware-ref <tag-or-commit>`, if claiming complete hardware validation for a release
- [ ] `cargo make rmk-behavior-tests`
- [ ] `cargo check --release --bin central`
- [ ] `cargo check --release --bin peripheral`
- [ ] `cargo build --release`
- [ ] `cargo make uf2 --release`, if release artifacts or flashing changed
- [ ] `python3 tools/firmware_artifact_manifest.py --require-uf2 > firmware-artifacts.local.json`, if release artifacts or flashed files changed

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
