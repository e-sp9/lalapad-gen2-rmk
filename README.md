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
cargo make reset-uf2 --release
```

This build runs a flash-layout guard that fails if the linked application or
generated UF2 files overlap RMK's on-chip storage region.

The generated files are:

- `firmware/normal/lalapad-gen2-rmk-central.uf2` for the right half
- `firmware/normal/lalapad-gen2-rmk-peripheral.uf2` for the left half
- `firmware/reset/lalapad-gen2-rmk-reset-central.uf2` for clearing right-half storage
- `firmware/reset/lalapad-gen2-rmk-reset-peripheral.uf2` for clearing left-half storage

After building UF2 files, generate a local hash manifest for the exact files
that will be flashed or referenced in hardware evidence:

```shell
python3 tools/firmware_artifact_manifest.py --require-uf2 --require-reset-uf2 > firmware-artifacts.local.json
```

For a clean local build, prefer the current-ref helper. It writes the manifest
to `firmware-artifacts.local.json`, or to `FIRMWARE_ARTIFACT_MANIFEST` when
that environment variable is set, pre-fills `firmware_ref` from the current
exact tag or short commit, and refuses mutable working-tree state:

```shell
cargo make firmware-artifact-manifest-current
```

Reset/storage-clear UF2 files are built from the same central/peripheral bins
with RMK `clear_storage = true` injected through `KEYBOARD_TOML_PATH`.

## Validation

```shell
python3 -m json.tool vial.json >/tmp/lalapad-vial-check.json
python3 -c 'import tomllib; [tomllib.load(open(path, "rb")) for path in ("keyboard.toml", "Cargo.toml", "Makefile.toml", "tools/porting_coverage_manifest.toml", "tools/porting_coverage_baseline.toml", "tools/hardware_validation_manifest.toml", "tools/hardware_validation_baseline.toml", "tools/hardware_validation_evidence.example.toml")]; print("toml ok")'
rmkit get-chip --keyboard-toml-path keyboard.toml
rmkit get-project-name --keyboard-toml-path keyboard.toml
cargo make porting-coverage
cargo make migration-status
cargo make migration-status-release-ready
cargo make migration-status-report
cargo make host-parity-tests
cargo make rmk-zmk-scenario-tests
cargo make rmk-behavior-tests
python3 tools/hardware_validation.py --hardware-baseline tools/hardware_validation_baseline.toml --require-classified
python3 tools/hardware_validation.py --markdown
python3 tools/hardware_validation.py --checklist
python3 tools/hardware_validation.py --evidence-template
python3 tools/check_flash_layout.py --config-only
python3 tools/check_flash_layout.py --require-reset-uf2
cargo make reset-uf2
cargo check --release --bin central
cargo check --release --bin peripheral
cargo build --release
```

When preparing a clean current commit for a hardware bench session, run:

```shell
cargo make hardware-validation-session-current
```

For manual or release-specific evidence preparation, the equivalent individual
steps are:

```shell
cargo make firmware-artifact-manifest-current
firmware_ref="$(git describe --tags --exact-match 2>/dev/null || git rev-parse --short=12 HEAD)"
artifact_pair_sha256="$(python3 -c 'import json; print(json.load(open("firmware-artifacts.local.json"))["pair_sha256"])')"
python3 tools/hardware_validation.py --evidence-template \
  --firmware-ref-template "$firmware_ref" \
  --artifact-pair-sha256-template "$artifact_pair_sha256" \
  --firmware-artifact-manifest-template firmware-artifacts.local.json \
  > hardware-validation-evidence.local.toml
python3 tools/hardware_validation.py --checklist \
  --firmware-ref-template "$firmware_ref" \
  --artifact-pair-sha256-template "$artifact_pair_sha256" \
  --firmware-artifact-manifest-template firmware-artifacts.local.json \
  > hardware-validation-checklist.local.md
python3 tools/hardware_validation.py --evidence hardware-validation-evidence.local.toml --markdown
```

For a manually supplied firmware ref instead of the current clean commit, use
`python3 tools/firmware_artifact_manifest.py --require-uf2 --require-reset-uf2 > firmware-artifacts.local.json`
and pass the same immutable ref through the template commands.
`cargo make hardware-validation-evidence-template-current` remains available
when a firmware-ref-only evidence template is useful before artifact hashing.

`cargo make porting-coverage` first runs the RMK host-runtime thumb layer-tap
scenario suite, the project host parity test suite, and the vendored RMK
behavior regression suite, then
`tools/porting_coverage.py` reads
`tools/porting_coverage_manifest.toml` and,
when the upstream ZMK checkout from the manifest is present, also parses
`config/lalapadgen2.keymap`, shield overlays, ZMK Kconfig values, and selected
RMK Rust constants to verify that the migration contract still matches the
source firmware. It also tracks the RMK host-runtime inventory for all golden
Space, Enter, and system tri-layer scenarios in
`vendor/rmk-0.8.2/tests/keyboard_lalapad_zmk_scenarios_test.rs`, including
non-HID system-layer actions that must not fall through to lower-layer keyboard
reports. Runtime scenario inventory checks are scoped to each test function so
an expected action in one scenario cannot accidentally satisfy another scenario.
It reports both migration-contract coverage and an explicit IQS9151 symbol
implementation status summary. `--require-porting-complete`
makes both metrics hard gates, so release builds fail if any source item is
unclassified or any explicit implementation status is non-implemented. The
`cargo make migration-status*` entrypoints also run the same RMK ZMK-derived
runtime scenario suite, project host parity tests, and vendored RMK behavior
regression suite before reporting a migration percentage, so a stale
Space/Enter/system tri-layer or RMK tap/hold behavior regression cannot be
hidden by static coverage alone. The
vendored RMK behavior suite uses `--tests`, so it intentionally re-runs the
LaLaPad scenario test binary while also covering the broader upstream RMK host
regression suite. The
text and JSON reports also include coverage grouped by result kind, making it
clear whether a regression is in RMK keymap/config checks, ZMK source
inventory, DTS/Kconfig mirrors, Rust constants, or firmware code needles. The
same reports include the ZMK keymap path, source availability, Git repository
path, Git commit, dirty state, and dirty path list so a `100.00%` porting
result is tied to the exact upstream checkout used as the source contract. The
combined release-readiness line stays failed when that checkout is dirty or
outside Git, even if the source-backed software coverage percentage is 100%.
Use `cargo make migration-status-release-ready` when that release-readiness line
must be a hard local or CI gate before a hardware-complete claim.
The
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
trackpad-to-position behavior and input-processor properties, IQS9151 runtime
input button code mapping and virtual-button positions against `keyboard.toml`
actions and Vial exposure, ZMK
split-input container properties, RMK split central/peripheral matrix
footprint, pin order, top-level matrix/serial absence, and controller topology,
ZMK trackpad listener device and normal/low-speed
input-processor chains, selected
ZMK DTS properties for split input, dynamic scaling, right-half column offset,
overlay trackpad routing overrides, matrix transform, physical-layout, I2C,
RGB LED, and charge-indicator nodes, ZMK GPIO pin flags, GPIO flag-to-target
polarity mirrors, the ZMK DTS/overlay status-node inventory, exact RMK keymap
array shape, exact RMK central/peripheral Cargo binary entries and default
feature preservation, cargo-make UF2/flash-layout task wiring, Release/Page
workflow DFU artifact names, web-flasher bundled firmware paths, Vial-exposed
host enablement, unlock chord, thumb layer-tap semantics, and the exact RMK combo
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
mandatory in another checkout layout. Source-backed monitoring gates also use
`--require-zmk-source-commit`, so the resolved ZMK checkout must be at the
manifest-pinned `metadata.source_commit`, not just any source tree. The
firmware CI checks out
`e-sp9/zmk-config-LalaPadGen2` at that manifest-pinned commit, parses
`vial.json`, RMK/Cargo/manifest TOML, and flash layout, runs this
source-backed complete-porting gate, and then runs the host-side parity test
suite before building release binaries. The local
host parity, ZMK scenario, and vendored RMK behavior test tasks run with
`RUSTFLAGS=-Dwarnings`, and the porting manifest checks that this warning-free
gate remains attached to the migration denominator.

`tools/migration_status.py` combines the source-backed migration gate and the
real-hardware validation tracker into a single release dashboard. In normal CI
it must show software coverage and implementation at `100.00%`, while hardware
validation can remain below 100% as long as every hardware-only check is still
classified. Its Markdown report includes hardware area, side, and remaining
check tables so release review can see which real-device evidence is still
missing. If a check is marked `validated` but its evidence is incomplete,
stale, or not tied to the flashed artifact hash, it is shown as
`validated_invalid` in the remaining table and does not increase the validated
count. Use `--require-hardware-validated --require-firmware-ref
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
so trackpad, split, Vial, status LED, battery, and storage gaps remain visible,
and they list each check's required observations next to the evidence request.
Real hardware results can be kept in a separate overlay file using the format
shown in `tools/hardware_validation_evidence.example.toml` and passed with
`--evidence path/to/evidence.toml`; this lets the manifest remain the stable
requirement list while measured evidence drives the validation rate.
Markdown reports include the retained `artifact_paths` values for each check,
so release review can tie validation progress back to the captured bench files.
Every hardware check declares non-empty `evidence_needles`; the classified
hardware gate fails if a check lacks those required observation terms.
Validated evidence must include `validated_at`, `tester`, `firmware_ref`, and
`artifact_or_notes`. `validated_at` must be a real `YYYY-MM-DD` date that is
not in the future. `firmware_ref` is compared as an exact string, usually a
release tag or commit hash for the central/peripheral firmware pair flashed
during the test; placeholders or moving refs such as `main`, `latest`, or
`HEAD` are rejected. Simulated, synthetic, mock, or host-only output cannot be
marked as validated hardware evidence. `artifact_or_notes` must contain a
concrete photo, log, probe, Vial observation, or similar measured evidence note,
and it must mention the check-specific observation terms declared by
`evidence_needles`. Use
`--require-firmware-ref <tag-or-commit>` to reject stale validated evidence;
combine it with `--require-validated` when a release needs all hardware checks
proven for that exact firmware. `--require-validated` also requires the
generated `metadata.hardware_check_inventory_sha256` and existing non-empty
`artifact_paths` files plus matching `artifact_path_sha256` hashes for every
counted validated entry. The
`--evidence-template` command generates a complete local overlay file
containing every current hardware check; pass
`--firmware-ref-template <tag-or-commit>` with it to pre-fill the flashed
firmware reference in every entry. Pass
`--artifact-pair-sha256-template <sha256>` when you also want each evidence note
seeded with the exact firmware artifact pair hash. Add
`--firmware-artifact-manifest-template firmware-artifacts.local.json` to bind
the generated template/checklist to the manifest and print the exact UF2/HEX
paths, sides, roles, sizes, and SHA256 hashes for the bench session. Generated
`--checklist` output turns the same manifest into a bench checklist for
collecting those observations. `cargo make
hardware-validation-evidence-template-current` fills the template from the
current tag or commit and refuses to run with tracked or untracked non-ignored
changes, which avoids recording evidence against a mutable local build.
`tools/firmware_artifact_manifest.py --require-uf2 --require-reset-uf2`
records the normal and storage-clear UF2 file sizes and SHA256 hashes, and
rejects files that do not have valid UF2 magic, payload size, block numbering,
and declared block count, so `artifact_or_notes` can point to an exact artifact
set.
`cargo make firmware-artifact-manifest-current` records the same hashes in
`firmware-artifacts.local.json`, or in `FIRMWARE_ARTIFACT_MANIFEST` when set,
and requires a clean current tag or commit for its `firmware_ref`.
`cargo make
hardware-validation-session-current` runs the RMK ZMK-derived runtime
scenarios, project host parity tests, RMK behavior regression suite, and the
current-ref firmware artifact manifest task, then writes
`hardware-validation-evidence.local.toml`,
`hardware-validation-checklist.local.md`, and `migration-status.local.md` for a
single hardware bench session; its evidence overlay is prefilled with both the
current firmware ref and the generated artifact `pair_sha256`, and its
checklist repeats the same identifiers plus the individual firmware artifact
paths and hashes so bench notes stay tied to the exact artifact set. Generated
`hardware-validation-evidence*.toml`,
`hardware-validation-checklist*.md`, `migration-status*.md`, and
`firmware-artifacts*.json` files are ignored by default.
When `--firmware-artifact-manifest` is supplied to a migration-status command,
every validated hardware evidence note must mention that manifest's
`pair_sha256`, even for partial evidence dashboards. This keeps each counted
hardware observation tied to the exact normal/reset UF2 files that were
flashed. The dashboard also checks that UF2, Intel HEX, and DFU files have the
expected artifact format, including Intel HEX checksums and EOF records. Plain
report commands render these problems in the
error list; use `--require-hardware-classified` or the final cargo-make gates
when the command must fail on stale or incomplete artifact evidence.
For final validation, each validated evidence entry must also list at least one
non-empty real file in `artifact_paths`. These retained evidence paths must be
relative paths under `hardware-evidence/`, resolved from
`EVIDENCE_ARTIFACT_ROOT` when that environment variable is set, or the current
directory otherwise. Use those files for the captured videos, photos, logs, or
scope traces referenced by `artifact_or_notes`; the path types must also match
the check's required artifacts, so a `video` check needs a video file, a
`Vial screenshot` check needs an image file, and a `BLE trace` check needs a
trace or log file. Each retained file must be named by path or basename in
`artifact_or_notes`, and separate required artifact types need separate files;
listing the same resolved file path twice does not count as separate evidence.
Each retained file must also have a matching `artifact_path_sha256` entry, so a
later file replacement at the same path invalidates the evidence instead of
silently preserving a 100% validation claim.
After copying captured evidence files into `artifact_paths`, run
`python3 tools/hardware_validation.py --evidence path/to/evidence.toml --evidence-artifact-root . --evidence-with-artifact-hashes`
to print the same evidence overlay with `artifact_path_sha256` populated from
the retained files. The same helper is available as
`HARDWARE_EVIDENCE=path/to/evidence.toml EVIDENCE_ARTIFACT_ROOT=. cargo make hardware-validation-hash-evidence`.
Write this output to a separate file and inspect it before replacing the
original evidence overlay.
To generate that hashed overlay and immediately run the complete final gate,
use:

```shell
HARDWARE_EVIDENCE=path/to/evidence.toml FIRMWARE_REF=tag-or-commit FIRMWARE_ARTIFACT_MANIFEST=firmware-artifacts.local.json EVIDENCE_ARTIFACT_ROOT=. cargo make hardware-validation-finalize-evidence
```

For evidence captured against the current clean commit, use:

```shell
HARDWARE_EVIDENCE=hardware-validation-evidence.local.toml EVIDENCE_ARTIFACT_ROOT=. cargo make hardware-validation-finalize-current
```

Media and trace file extensions are also checked against their file signatures,
so a text file renamed to `.mp4`, `.png`, `.jpg`, `.webp`, `.pcap`, or
`.pcapng` does not count as retained hardware evidence.
The generated evidence template and bench checklist include deterministic
`hardware-evidence/<check-id>-<artifact-type>.<ext>` path suggestions for each
required artifact, assuming the default `EVIDENCE_ARTIFACT_ROOT=.`. The copy
aid includes those paths so the final gate can confirm the retained files are
actually referenced by the bench note, but its observation placeholder must be
replaced with real bench output before the check can count as validated.
For the combined final gate, run the cargo-make task so the RMK runtime
scenario suite runs before the migration dashboard is evaluated:

```shell
HARDWARE_EVIDENCE=path/to/evidence.toml FIRMWARE_REF=tag-or-commit FIRMWARE_ARTIFACT_MANIFEST=firmware-artifacts.local.json cargo make migration-status-final
```

`cargo make hardware-validation-finalize-evidence` is the same release path
with the retained-artifact hash overlay generated first. It writes
`hardware-validation-evidence.hashed.local.toml` by default, or the path from
`HASHED_HARDWARE_EVIDENCE`. The source evidence path is refused as the hashed
output path, so the wrapper cannot overwrite the original overlay before the
final gate passes.

It must report `Full validation: pass` before claiming source-backed and
real-device validation are both complete for that firmware. The final gate also
requires the resolved ZMK source checkout to be a readable, clean Git
repository at the manifest-pinned `metadata.source_commit`, so a release claim
cannot be tied to uncommitted upstream-source edits or to a different upstream
revision. The task uses `firmware-artifacts.local.json` by default, or
`FIRMWARE_ARTIFACT_MANIFEST` when set. It also rejects hardware evidence
overlays missing the generated `metadata.hardware_check_inventory_sha256`, or
whose hash no longer matches the current hardware validation manifest.

For hardware evidence collected from the current clean commit, this variant
derives the exact tag or short commit automatically and refuses tracked or
untracked non-ignored RMK changes, regenerates
the artifact manifest path used by the final gate from the current UF2 outputs,
and then runs the same final gate:

```shell
HARDWARE_EVIDENCE=hardware-validation-evidence.local.toml cargo make migration-status-final-current
```

`cargo make hardware-validation-finalize-current` performs the same current-ref
final gate after generating the hash-populated evidence overlay.

For a local Markdown dashboard matching the CI summary, run:

```shell
cargo make migration-status-report
```

When reviewing partial hardware evidence, include the overlay and flashed
firmware reference. Add `FIRMWARE_ARTIFACT_MANIFEST` to show and validate the
exact UF2 hash manifest in the dashboard:

```shell
HARDWARE_EVIDENCE=path/to/evidence.toml FIRMWARE_REF=tag-or-commit cargo make migration-status-report
```

Host-side parity tests can be run explicitly with:

```shell
cargo test --lib --target x86_64-unknown-linux-gnu
cargo make rmk-behavior-tests
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
