# LaLaPad Gen2 RMK Community Firmware

This repository contains an RMK-based community firmware for LaLaPad Gen2.

It rebuilds the upstream ZMK firmware behavior for Seeed Studio XIAO nRF52840 /
XIAO BLE while keeping the original split orientation: right half is central,
left half is peripheral.

This is not an official firmware from RMK, ZMK, Seeed Studio, or the original
LaLaPad Gen2 hardware project.

## Status

- Seeed Studio XIAO nRF52840 / XIAO BLE target
- BLE split keyboard
- Right half as central, left half as peripheral
- 42 keyboard keys plus the two 5-way switches
- Vial definition for the physical key matrix and trackpad virtual positions
- Keymap positions for the upstream trackpad button/gesture bindings
- Experimental IQS9151 cursor, scroll, tap/swipe, pinch, inertia, tap-drag, and dynamic scaling support
- Upstream BLE TX power, charge-indicator pin configuration, and RGB LED status widget behavior

Known remaining work:

- Hardware tuning for final IQS9151 axes, speed, thresholds, and battery behavior

Approximations and behavior differences are tracked in `docs/PORTING.md`.

## Flashing

Use matching left/right artifacts from the same release.

- Right half: `lalapad-gen2-rmk-central`
- Left half: `lalapad-gen2-rmk-peripheral`

Before testing over BLE, remove the old host pairing and pair again. Hosts can
cache the old HID report descriptor, especially after mouse/scroll firmware
changes.

## Bluetooth Pairing

This firmware enables RMK passkey entry for hosts that require authenticated
keyboard pairing. If the host shows a 6-digit passkey during pairing, type that
passkey on the keyboard and press Enter. During passkey entry, digit keys,
Enter, Escape, and Backspace are captured by the firmware and are not sent as
normal host keypresses.

On the default keymap, use the numeric layer for digit entry. The left side has
`1`-`5` on the first row and `6`-`0` on the second row. The right side matches
the upstream ZMK numpad block on the secondary layer.

The system layer is selected by holding both secondary and tertiary layer keys.
It exposes Bluetooth profile selection, output toggle, reset/bootloader, and
the trackpad dynamic-scale controls.
While the secondary, tertiary, or system tri-layer is active, the trackpad uses
the upstream ZMK low-speed divisors for cursor and scroll movement.

### Web Flasher

For convenience, this repository includes a lightweight browser-based flasher
under `tools/web-flasher/`. This is not an RMK-specific flashing path; it is a
small helper for writing this firmware to the XIAO BLE without installing local
flashing tools. GitHub Actions deploys it to GitHub Pages on each push to
`main` and each published release.

- Supported browsers: Chrome / Edge / Opera (requires WebHID + WebSerial)
- How it works: a Nordic Legacy DFU client talks directly to the Adafruit nRF52
  UF2 bootloader bundled with the XIAO BLE
- Flash both halves because the left and right firmware images are different
  (the UI is ordered left-to-right: Peripheral, then Central)
- The right half can enter DFU mode automatically over WebHID. The left half has
  no USB HID interface, so double-tap the reset button to enter DFU mode
  manually.

Release builds attach
`firmware/lalapad-gen2-rmk-{central,peripheral}-dfu.zip` to the GitHub Release
and bundle those files into the Pages deployment. You can also load local ZIP
files in the flasher UI.

### UF2

Install the RMK toolchain pieces described in the RMK local compilation guide:

```shell
cargo install flip-link cargo-make
rustup target add thumbv7em-none-eabihf
```

Build ELF binaries:

```shell
cargo build --release
```

Build UF2 files for the XIAO BLE / Adafruit nRF52 bootloader:

```shell
cargo make uf2 --release
```

This build runs a flash-layout guard that fails if the linked application or
generated UF2 files overlap RMK's on-chip storage region.

The generated files are:

- `firmware/normal/lalapad-gen2-rmk-central.uf2` for the right half
- `firmware/normal/lalapad-gen2-rmk-peripheral.uf2` for the left half

Reset/storage-clear UF2 files, when generated for hardware testing, are kept under `firmware/reset/`.

## Validation

```shell
python3 -m json.tool vial.json >/tmp/lalapad-vial-check.json
python3 -c 'import tomllib; tomllib.load(open("keyboard.toml", "rb")); tomllib.load(open("Cargo.toml", "rb")); print("toml ok")'
rmkit get-chip --keyboard-toml-path keyboard.toml
rmkit get-project-name --keyboard-toml-path keyboard.toml
python3 tools/check_flash_layout.py --config-only
cargo check --release --bin central
cargo check --release --bin peripheral
cargo build --release
```

Host-side parity tests can be run explicitly with:

```shell
cargo test --lib --target x86_64-unknown-linux-gnu
```

Run `cargo make uf2 --release` when changing release artifacts or flashing behavior.

## Documentation

- `AGENTS.md`: coding-agent project guide and repository working rules
- `docs/PORTING.md`: ZMK-to-RMK porting notes, pin mapping, and behavior differences
- `docs/TRACKPAD_HARDWARE_CHECK.md`: IQS9151 hardware validation checklist
- `docs/RELEASE.md`: release, artifact, and flashing checklist
- `docs/COMMUNITY_ANNOUNCEMENT.md`: community announcement draft

## Sources Used For Porting

- RMK current documentation and `nrf52840_split` template
- `ShiniNet/LaLaPadGen2`
- `ShiniNet/zmk-config-LalaPadGen2`

See `docs/PORTING.md` for pin mapping, upstream parity notes, and remaining firmware gaps.

## Contributing

Hardware reports and focused pull requests are welcome. See `CONTRIBUTING.md`
before opening a PR.

## License

Licensed under either Apache-2.0 or MIT, at your option.
