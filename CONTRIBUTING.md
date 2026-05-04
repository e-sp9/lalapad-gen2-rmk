# Contributing

Thanks for helping improve the RMK firmware for LaLaPad Gen2.

This repository is a community firmware port. It is not an official firmware
from RMK, ZMK, Seeed Studio, or the original LaLaPad Gen2 hardware project.

## What To Report

Useful reports include:

- build failures on a clean checkout
- flashing failures with the generated UF2 or DFU zip files
- left/right split pairing problems
- IQS9151 cursor, scroll, pinch, tap, tap-drag, or gesture issues
- RGB LED, charge LED, or battery behavior differences
- keymap, Vial, combo, or BLE profile behavior differences from the upstream ZMK firmware

When reporting hardware behavior, include:

- firmware version, commit, or release tag
- which half is affected: left peripheral, right central, or both
- USB or BLE connection
- host OS and browser, when using the web flasher
- whether the old BLE pairing was removed before testing
- a short reproduction sequence

## Development Setup

Install the RMK build requirements:

```sh
cargo install flip-link cargo-make
rustup target add thumbv7em-none-eabihf
```

`nrf-sdc` uses bindgen and needs `libclang`. On machines without a system
`libclang`, use a local virtual environment or install `libclang-dev` through
the system package manager. Do not commit local helper environments.

## Validation

Run lightweight validation first:

```sh
python3 -m json.tool vial.json >/tmp/lalapad-vial-check.json
python3 -c 'import tomllib; tomllib.load(open("keyboard.toml", "rb")); tomllib.load(open("Cargo.toml", "rb")); print("toml ok")'
rmkit get-chip --keyboard-toml-path keyboard.toml
rmkit get-project-name --keyboard-toml-path keyboard.toml
```

Then run firmware checks:

```sh
cargo check --release --bin central
cargo check --release --bin peripheral
cargo build --release
```

When changing generated release artifacts or flashing behavior, also run:

```sh
cargo make uf2 --release
```

## Pull Requests

- Keep changes focused.
- Do not commit generated firmware artifacts, `target/`, local virtualenvs, or bindgen helper headers.
- Preserve the upstream ZMK split orientation unless the change explicitly documents why it differs: right half is central, left half is peripheral.
- Document hardware assumptions and behavior differences in `docs/PORTING.md`.
- Update `docs/TRACKPAD_HARDWARE_CHECK.md` when changing trackpad diagnostics or expected behavior.
- Add issue reproduction notes or hardware test results when the change depends on real hardware behavior.

## Licensing

By contributing, you agree that your contribution is licensed under either MIT
or Apache-2.0, at the recipient's option, matching this repository.
