# Coding Agent Instructions

This file is the canonical project guide for coding agents working in this
repository. Keep `CLAUDE.md` and `.github/copilot-instructions.md` as thin
pointers to this file instead of duplicating implementation status there.

## Project Snapshot

This repository contains an RMK-based community firmware for LaLaPad Gen2.

Current target:

- Board: Seeed Studio XIAO nRF52840 / XIAO BLE
- Firmware framework: RMK 0.8 line, with a local patch at `vendor/rmk-0.8.2`
- Topology: BLE split keyboard
- Orientation: right half is central, left half is peripheral
- Matrix scope: 42 keyboard keys plus the two 5-way switches
- Pointing scope: experimental IQS9151 cursor, scroll, tap/swipe, pinch,
  inertia, tap-drag, and dynamic scaling support
- UI/config scope: Vial layout plus keymap positions for upstream trackpad
  button and gesture bindings
- Status widget scope: BLE TX power, charge-indicator pins, and local RGB LED
  battery/connection/layer status behavior

Known remaining work is hardware-centered: final IQS9151 axis, speed, threshold,
RGB polarity, and battery behavior tuning on real hardware. Track
approximations and behavior differences in `docs/PORTING.md`.

This is not an official firmware from RMK, ZMK, Seeed Studio, or the original
LaLaPad Gen2 hardware project.

## Source Of Truth

Treat RMK as version-sensitive. Before changing RMK-specific code,
configuration, dependencies, build commands, split behavior, Vial behavior, or
input-device behavior, check current official sources:

- https://rmk.rs/main/docs/
- https://github.com/HaoboGu/rmk
- https://github.com/HaoboGu/rmk-template
- https://docs.rs/rmk/latest/rmk/

Use RMK docs for user-facing configuration and workflow behavior. Use RMK
GitHub examples/templates when docs are ambiguous or when a feature is
unreleased. Keep dependency versions internally consistent with the selected
RMK release; do not switch this project to RMK `main` unless the task explicitly
requires an unreleased feature.

Use these local files as the project source of truth:

- `keyboard.toml`: RMK keyboard, split, matrix, BLE, and keymap configuration
- `vial.json`: host-side Vial layout definition
- `src/central.rs`: right-half central firmware entrypoint
- `src/peripheral.rs`: left-half peripheral firmware entrypoint
- `src/iqs9151.rs`: IQS9151 driver, gesture recognizer, pointer reports, and
  split transport shim
- `src/rgb_widget.rs`: local RGB LED status widget controller
- `src/lib.rs`: shared custom events and RMK integration hooks
- `docs/PORTING.md`: ZMK-to-RMK porting notes, pin mapping, and known gaps
- `docs/TRACKPAD_HARDWARE_CHECK.md`: hardware validation checklist
- `README.md`: build, flashing, and repository overview

## Development Rules

- Check `git status --short --branch` before editing.
- Preserve the upstream ZMK orientation unless asked otherwise: right side
  central, left side peripheral.
- Convert source firmware pin names through the authoritative board pin map
  before writing RMK pin names.
- Keep matrix positions, virtual trackpad positions, and Vial layout positions
  aligned. Trackpad virtual rows are not GPIO-scanned rows.
- Document approximations and unsupported or hardware-dependent behavior,
  especially for pointing devices, battery reporting, and status LEDs.
- Do not remove the local `vendor/rmk-0.8.2` patch casually; it carries HID
  report map changes needed for the composite mouse high-resolution wheel/pan
  behavior.
- Do not commit generated firmware artifacts, `target/`, `.venv-libclang`,
  `.bindgen-headers`, `.codex`, generated UF2 files, generated ELF/HEX/BIN
  files, web-flasher firmware ZIPs, or `tools/web-flasher/firmware/`.
- Leave unrelated user changes alone. If a file has unrelated edits, work with
  them instead of reverting them.

## Build Verification

Run lightweight validation first:

```sh
python3 -m json.tool vial.json >/tmp/lalapad-vial-check.json
python3 -c 'import tomllib; [tomllib.load(open(path, "rb")) for path in ("keyboard.toml", "Cargo.toml", "Makefile.toml", "tools/porting_coverage_manifest.toml", "tools/porting_coverage_baseline.toml", "tools/hardware_validation_manifest.toml", "tools/hardware_validation_evidence.example.toml")]; print("toml ok")'
rmkit get-chip --keyboard-toml-path keyboard.toml
rmkit get-project-name --keyboard-toml-path keyboard.toml
python3 tools/check_flash_layout.py --config-only
```

Run firmware checks for code, config, dependency, or behavior changes:

```sh
cargo check --release --bin central
cargo check --release --bin peripheral
cargo build --release
```

Build UF2 artifacts when requested or when changing release/flashing behavior:

```sh
cargo make uf2 --release
```

`cargo make uf2 --release` also runs `tools/check_flash_layout.py
--require-uf2` after UF2 generation. Treat any application/storage overlap as a
release blocker.

For docs-only changes, at minimum run:

```sh
git diff --check
```

If a build cannot be completed because of local toolchain setup, say that
clearly and distinguish it from a firmware/configuration failure.

## Local Toolchain Notes

`nrf-sdc` uses bindgen and requires `libclang`. Prefer a normal system
`clang`/`libclang` installation when available. If the machine has no system
`libclang`, a project-local helper environment can be used without committing
it:

```sh
python3 -m venv .venv-libclang
.venv-libclang/bin/pip install libclang
```

In constrained environments where bindgen cannot find freestanding C headers,
use ignored local helper headers under `.bindgen-headers/` and run:

```sh
LIBCLANG_PATH="$PWD/.venv-libclang/lib/python3.12/site-packages/clang/native" \
BINDGEN_EXTRA_CLANG_ARGS="-nostdinc -I$PWD/.bindgen-headers" \
cargo build --release
```

## Common Task Guidance

- Trackpad work: start from `docs/PORTING.md` and `src/iqs9151.rs`; preserve
  ZMK-derived virtual button ordering for pinch and gesture events.
- Split behavior work: check both `src/central.rs` and `src/peripheral.rs`;
  left-side pointer traffic crosses the split link before host reporting.
- RGB/battery work: check `src/rgb_widget.rs`, `keyboard.toml`, and
  `docs/PORTING.md`; verify assumptions on hardware before declaring parity.
- Web flasher work: check `tools/web-flasher/`, `.github/workflows/`, and
  `docs/RELEASE.md`; do not commit generated firmware payloads.
- Release work: follow `docs/RELEASE.md` and keep README flashing instructions
  consistent with generated artifacts.

## Git Hygiene

- Stage only files relevant to the task.
- Commit only after a successful relevant validation command, or after clearly
  documenting why validation could not be completed.
- Do not make the repository public, change repository visibility, or publish a
  release unless the user explicitly asks for that action.
