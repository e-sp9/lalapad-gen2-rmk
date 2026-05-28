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
- [ ] `python3 -c 'import tomllib; tomllib.load(open("keyboard.toml", "rb")); tomllib.load(open("Cargo.toml", "rb")); print("toml ok")'`
- [ ] `rmkit get-chip --keyboard-toml-path keyboard.toml`
- [ ] `rmkit get-project-name --keyboard-toml-path keyboard.toml`
- [ ] `python3 tools/porting_coverage.py --require-zmk-source`
- [ ] `cargo check --release --bin central`
- [ ] `cargo check --release --bin peripheral`
- [ ] `cargo build --release`
- [ ] `cargo make uf2 --release`, if release artifacts or flashing changed

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
