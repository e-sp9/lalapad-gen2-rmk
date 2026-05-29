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
python3 -c 'import tomllib; [tomllib.load(open(path, "rb")) for path in ("keyboard.toml", "Cargo.toml", "Makefile.toml", "tools/porting_coverage_manifest.toml", "tools/porting_coverage_baseline.toml", "tools/hardware_validation_manifest.toml", "tools/hardware_validation_baseline.toml", "tools/hardware_validation_evidence.example.toml")]; print("toml ok")'
rmkit get-chip --keyboard-toml-path keyboard.toml
rmkit get-project-name --keyboard-toml-path keyboard.toml
python3 tools/porting_coverage.py --coverage-baseline tools/porting_coverage_baseline.toml --require-zmk-source --require-porting-complete
python3 tools/migration_status.py --coverage-baseline tools/porting_coverage_baseline.toml --hardware-baseline tools/hardware_validation_baseline.toml --require-zmk-source --require-software-complete --require-hardware-classified
python3 tools/hardware_validation.py --hardware-baseline tools/hardware_validation_baseline.toml --require-classified
python3 tools/hardware_validation.py --markdown
python3 tools/hardware_validation.py --evidence-template
python3 tools/check_flash_layout.py --config-only
cargo check --release --bin central
cargo check --release --bin peripheral
cargo build --release
```

When real hardware evidence changed, also run:

```shell
python3 tools/hardware_validation.py --evidence-template > hardware-validation-evidence.local.toml
python3 tools/hardware_validation.py --evidence hardware-validation-evidence.local.toml --markdown
```

`tools/porting_coverage.py` reads `tools/porting_coverage_manifest.toml` and,
when the upstream ZMK checkout from the manifest is present, also parses
`config/lalapadgen2.keymap`, shield overlays, ZMK Kconfig values, and selected
RMK Rust constants to verify that the migration contract still matches the
source firmware. It reports both migration-contract coverage and an explicit
IQS9151 symbol implementation status summary. `--require-porting-complete`
makes both metrics hard gates, so release builds fail if any source item is
unclassified or any explicit implementation status is non-implemented. The
text and JSON reports also include coverage grouped by result kind, making it
clear whether a regression is in RMK keymap/config checks, ZMK source
inventory, DTS/Kconfig mirrors, Rust constants, or firmware code needles. The
`--coverage-baseline tools/porting_coverage_baseline.toml` gate also freezes
the current denominator, result-id inventory hash, and per-kind totals, so
deleting or swapping coverage items cannot silently turn into a smaller
`100.00%`. `--hardware-baseline tools/hardware_validation_baseline.toml`
similarly freezes the real-hardware validation check inventory, area/side
counts, and default status counts so hardware-only gaps cannot be deleted or
renamed to manufacture a smaller final-validation denominator. The Rust checks
cover the RMK-side IQS9151 register-address
inventory, upstream IQS9151 register and bit-flag porting classifications,
product/register address values, reset/gesture bits, IQS9151 feature-enable
flags, dynamic-scale bounds, timing values, and initialization byte-array
checksums. It also checks left/right IQS9151 Kconfig parity and the
ZMK trackpad listener routing shape, verifies that every active ZMK Kconfig key
in the source files is classified by the migration contract, and checks Vial's
matrix positions, ZMK `default_transform` order, the active ZMK physical-layout
chain and physical key attributes, ZMK repo-level and config source-file
inventories, upstream ZMK workflow and build-matrix files, ZMK layout JSON
metadata and per-key coordinates, ZMK keymap layer and behavior inventories,
ZMK behavior-node and combo-node property inventories, ZMK include inventory,
active and disabled Kconfig lines, Kconfig shield/default entries, DTS aliases
and DTS root/overlay node inventories,
west module inventory, ZMK trackpad virtual-position
defines, RMK custom keycode order and labels, Vial name/VID/PID identity and
Vial serial-number prefix, ZMK `INPUT_BTN_*` to virtual-position bindings, ZMK
trackpad-to-position behavior and input-processor properties, ZMK split-input
container properties, ZMK trackpad listener device and normal/low-speed
input-processor chains, selected
ZMK DTS properties for split input, dynamic scaling, right-half column offset,
overlay trackpad routing overrides, matrix transform, physical-layout, I2C,
RGB LED, and charge-indicator nodes, ZMK GPIO pin flags, the ZMK DTS/overlay
status-node inventory, exact RMK keymap array shape, and the exact RMK combo
inventory. It also freezes both the Cargo dependency resolution that keeps the
local RMK patch active and the patch invariants for the HID descriptor, BLE
mouse feature report, high-resolution wheel, horizontal pan handling, and
LaLaPad dynamic-scale storage persistence. The
source-backed gate uses structured inventories instead of regex-only ZMK source
checks for these high-risk DTS, GPIO, trackpad, and split-routing details, and
it cross-checks thumb tap/hold layer-resolution scenarios against the ZMK
source keymap after documented RMK deltas are applied, including deriving each
thumb hold's active layer from the `LT(...)`/`MO(...)` action instead of trusting
the scenario metadata alone. It also checks that
Vial's `customKeycodes` names, titles, and short labels match RMK's
`User0..User13` BLE and dynamic-scale handler semantics, so host-side remapping
cannot silently point at the wrong firmware action. The Vial identity gate also
checks that `vial.json` still targets the same `keyboard.toml` name, VID, and
PID, and that the firmware serial number keeps RMK's Vial recognition prefix.
The Vial position gate also compares `vial.json` against
`keyboard.toml` bounds, all non-empty firmware key positions, and the explicitly
classified no-action physical positions without needing
the upstream ZMK checkout. It also resolves every
position on layer 1, layer 2, and the system tri-layer against the ZMK source
keymap to catch transparent-key fallthrough drift. ZMK hold-tap timing values
are also mirrored against the RMK Morse timing settings. Use
`--zmk-keymap PATH --require-zmk-source` when the source-backed check must be
mandatory in another checkout layout. The firmware CI checks out
`e-sp9/zmk-config-LalaPadGen2`, parses `vial.json`, RMK/Cargo/manifest TOML,
and flash layout, runs this source-backed complete-porting gate, and then runs
the host-side parity test suite before building release binaries.

`tools/migration_status.py` combines the source-backed migration gate and the
real-hardware validation tracker into a single release dashboard. In normal CI
it must show software coverage and implementation at `100.00%`, while hardware
validation can remain below 100% as long as every hardware-only check is still
classified. Its Markdown report includes hardware area, side, and remaining
check tables so release review can see which real-device evidence is still
missing. Use `--require-hardware-validated --require-firmware-ref
<tag-or-commit>` only when claiming that a specific flashed firmware has passed
every real-device check.

`tools/hardware_validation.py` reads
`tools/hardware_validation_manifest.toml` and tracks checks that cannot be
proven by the static source-backed porting gate: IQS9151 electrical identity,
RDY behavior, left/right trackpad runtime behavior, BLE split reconnection,
Vial thumb layer-tap behavior, RGB/battery indicators, and reset/reflash
behavior. CI runs it with `--require-classified` so every hardware-only item
must stay explicitly tracked and linked to an existing Markdown source heading,
but it does not use `--require-validated` because real hardware evidence cannot
be created by GitHub Actions. The CI job also writes
`python3 tools/hardware_validation.py --markdown` to the GitHub Actions step
summary so remaining real-device evidence is visible next to each release
build. Reports include overall, area-level, and side-level validation progress
so trackpad, split, Vial, status LED, battery, and storage gaps remain visible.
Real hardware results can be kept in a separate overlay file using the format
shown in `tools/hardware_validation_evidence.example.toml` and passed with
`--evidence path/to/evidence.toml`; this lets the manifest remain the stable
requirement list while measured evidence drives the validation rate.
Validated evidence must include `validated_at`, `tester`, `firmware_ref`, and
`artifact_or_notes`. `validated_at` must be a real `YYYY-MM-DD` date that is
not in the future. `firmware_ref` is compared as an exact string, usually a
release tag or commit hash for the central/peripheral firmware pair flashed
during the test; placeholders or moving refs such as `main`, `latest`, or
`HEAD` are rejected. `artifact_or_notes` must contain a concrete photo, log,
probe, Vial observation, or similar measured evidence note. Use
`--require-firmware-ref <tag-or-commit>` to reject stale validated evidence;
combine it with `--require-validated` when a release needs all hardware checks
proven for that exact firmware. The `--evidence-template`
command generates a complete local overlay file containing every current
hardware check; pass `--firmware-ref-template <tag-or-commit>` with it to
pre-fill the flashed firmware reference in every entry. Generated
`hardware-validation-evidence*.toml` files are ignored by default.
For the combined final gate, run:

```shell
python3 tools/migration_status.py --coverage-baseline tools/porting_coverage_baseline.toml \
  --hardware-baseline tools/hardware_validation_baseline.toml \
  --evidence path/to/evidence.toml \
  --require-software-complete \
  --require-hardware-classified \
  --require-hardware-validated \
  --require-firmware-ref <tag-or-commit>
```

It must report `Full validation: pass` before claiming source-backed and
real-device validation are both complete for that firmware. The equivalent
cargo-make task is:

```shell
HARDWARE_EVIDENCE=path/to/evidence.toml FIRMWARE_REF=tag-or-commit cargo make migration-status-final
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
