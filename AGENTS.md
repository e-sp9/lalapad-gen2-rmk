# Agent Instructions

## Project

This repository contains an RMK firmware project for rebuilding LaLaPad Gen2 firmware.

Current target:

- Board: Seeed Studio XIAO nRF52840 / XIAO BLE
- Firmware framework: RMK
- Topology: BLE split keyboard
- Orientation: right half is central, left half is peripheral
- Implemented scope: GPIO key matrix and 5-way switches
- Not yet implemented: IQS9151 trackpads, gesture mapping, LED/battery widgets

## Source Of Truth

Treat RMK as version-sensitive. Before changing RMK-specific code, configuration, dependencies, or build commands, check current official sources:

- https://rmk.rs/main/docs/
- https://github.com/HaoboGu/rmk
- https://github.com/HaoboGu/rmk-template
- https://docs.rs/rmk/latest/rmk/

Use RMK docs for configuration behavior. Use RMK GitHub examples/templates when docs are ambiguous or when a feature is unreleased.

Use these local project files as porting context:

- `keyboard.toml`: RMK keyboard, split, matrix, and keymap configuration
- `vial.json`: Vial layout definition
- `docs/PORTING.md`: pin mapping, source-firmware notes, and known gaps
- `README.md`: build and flashing summary

## Development Rules

- Keep dependency versions internally consistent with the selected RMK release.
- Do not switch to RMK `main` unless the task explicitly requires an unreleased feature.
- Preserve the upstream ZMK orientation unless asked otherwise: right side central, left side peripheral.
- Convert source firmware pin names through the authoritative board pin map before writing RMK pin names.
- Document approximations and unsupported hardware behavior, especially for pointing devices.
- Do not commit generated firmware artifacts, `target/`, local virtualenvs, or bindgen helper headers.

## Build Verification

Run lightweight validation first:

```sh
python3 -m json.tool vial.json >/tmp/lalapad-vial-check.json
python3 -c 'import tomllib; tomllib.load(open("keyboard.toml", "rb")); tomllib.load(open("Cargo.toml", "rb")); print("toml ok")'
rmkit get-chip --keyboard-toml-path keyboard.toml
rmkit get-project-name --keyboard-toml-path keyboard.toml
```

Run firmware checks:

```sh
cargo check --release --bin central
cargo check --release --bin peripheral
cargo build --release
```

Build UF2 artifacts when requested:

```sh
cargo make uf2 --release
```

## Local Toolchain Notes

`nrf-sdc` uses bindgen and requires `libclang`. If the machine has no system `libclang`, a project-local helper environment can be used without committing it:

```sh
python3 -m venv .venv-libclang
.venv-libclang/bin/pip install libclang
```

In constrained environments where bindgen cannot find freestanding C headers, use ignored local helper headers under `.bindgen-headers/` and run:

```sh
LIBCLANG_PATH="$PWD/.venv-libclang/lib/python3.12/site-packages/clang/native" \
BINDGEN_EXTRA_CLANG_ARGS="-nostdinc -I$PWD/.bindgen-headers" \
cargo build --release
```

Prefer a normal system `clang`/`libclang` installation when available.

## Git Hygiene

- Check `git status --short --branch` before editing.
- Leave unrelated untracked files alone. In this repo, `.codex` may exist locally and should not be staged.
- Commit only after a successful relevant build or after clearly documenting why a build could not be completed.
