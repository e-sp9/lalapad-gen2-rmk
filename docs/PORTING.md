# LaLaPad Gen2 RMK Porting Notes

## RMK Baseline

The project was generated from RMK's `nrf52840_split` template and then adapted for LaLaPad Gen2.

RMK's current local compilation documentation recommends:

- `rmkit init` for creating a firmware project
- `keyboard.toml` for keyboard, matrix, split, and keymap configuration
- `vial.json` for the host-side Vial layout
- `cargo build --release` for ELF output
- `cargo make uf2 --release` for UF2 output

## Hardware Source

The hardware mapping is based on the upstream ZMK configuration:

- board: `seeeduino_xiao_ble`
- shield: `lalapadgen2_right` as central
- shield: `lalapadgen2_left` as peripheral
- matrix diode direction: `col2row`

## XIAO BLE Pin Mapping

Zephyr `xiao_d` names from the upstream ZMK config were converted to nRF pin names:

| XIAO name | nRF pin |
| --- | --- |
| D0 | P0_02 |
| D1 | P0_03 |
| D2 | P0_28 |
| D3 | P0_29 |
| D4 | P0_04 |
| D5 | P0_05 |
| D6 | P1_11 |
| D7 | P1_12 |
| D8 | P1_13 |
| D9 | P1_14 |
| D10 | P1_15 |

Shared row pins:

```toml
row_pins = ["P1_15", "P1_14", "P1_13", "P1_12", "P1_01"]
```

Right half / central columns:

```toml
col_pins = ["P0_02", "P0_03", "P0_28", "P0_29", "P0_15", "P0_19"]
```

Left half / peripheral columns:

```toml
col_pins = ["P0_19", "P0_15", "P0_29", "P0_28", "P0_03", "P0_02"]
```

## Matrix And Virtual Position Scope

The physical RMK split matrix currently scans rows `0..4` from the ZMK transform:

- rows `0..3`: main 42-key layout
- row `4`: left and right 5-way switches

The RMK keymap and Vial definition also include rows `5..6` as non-scanned virtual positions for the upstream trackpad button/gesture events:

| ZMK position | RMK row/col | Meaning |
| --- | --- | --- |
| 52 | `5,0` | left trackpad left click |
| 53 | `5,1` | left trackpad right click |
| 54 | `5,2` | left trackpad middle click |
| 55 | `5,9` | right trackpad left click |
| 56 | `5,10` | right trackpad right click |
| 57 | `5,11` | right trackpad middle click |
| 58 | `6,0` | left trackpad left gesture |
| 59 | `6,1` | left trackpad right gesture |
| 60 | `6,2` | left trackpad up gesture |
| 61 | `6,3` | left trackpad down gesture |
| 62 | `6,4` | left trackpad pinch gesture |
| 63 | `6,7` | right trackpad left gesture |
| 64 | `6,8` | right trackpad right gesture |
| 65 | `6,9` | right trackpad up gesture |
| 66 | `6,10` | right trackpad down gesture |
| 67 | `6,11` | right trackpad pinch gesture |

These positions are not GPIO rows. The IQS9151 runtime emits `KeyboardEvent::key(row, col, pressed)` for the mapped virtual positions after decoding Azoteq button and gesture events.

## Trackpad Gap

LaLaPad Gen2 uses `azoteq,iqs9151` in the upstream ZMK shield.

RMK's current main documentation includes a TOML-configurable Azoteq IQS5xx trackpad driver for IQS550 / IQS572 / IQS525 style devices. That is not the same device as IQS9151, so the LaLaPad Gen2 trackpad behavior should be treated as a separate porting task.

The upstream ZMK config pulls IQS9151 support from `ShiniNet/zmk-driver-iqs9151`. That driver reads the IQS9151 coordinate block at `0x1014..0x105b`, does gesture recognition in the firmware driver, and emits `INPUT_BTN_0..7` for click/gesture events. The ZMK input processor then consumes those button events and raises virtual key positions.

RMK-side groundwork now lives in `src/iqs9151.rs`:

- IQS9151 register constants for the product number, coordinate block, flags, and finger coordinates
- a generic `embedded-hal-async` I2C wrapper for reading the product number and coordinate block
- a parser for the coordinate block layout used by the upstream driver
- mapping from upstream `INPUT_BTN_0..7` semantics to RMK virtual rows `5..6`
- an edge tracker that converts a button bitmask into press/release events for those virtual positions
- conversion from those edge events into RMK `KeyboardEvent` values
- a coordinate-frame recognizer for one-finger tap/drag, two-finger tap/drag/scroll/pinch, three-finger tap/drag, and three-finger swipe events
- relative cursor movement from the IQS9151 `relative_x` / `relative_y` fields through HID mouse reports
- two-finger vertical/horizontal scroll from centroid movement through HID mouse `wheel` / `pan` reports
- two-finger pinch is routed as a ZMK-style frame result: the upstream `INPUT_BTN_7` virtual key is pressed first, pinch wheel deltas are emitted while it is held, and the key is released on pinch end. RMK custom events keep that key/wheel ordering across both the central and split paths so pinch does not degrade into plain scroll when the wheel report reaches the host before the virtual key. Pinch wheel direction, divisor, and gain now follow the upstream normal-mode processor shape rather than the earlier capped/reversed hardware-test approximation.
- two-finger mode selection follows the upstream driver: once scroll or pinch mode is selected, the mode stays fixed until the two-finger session ends.
- a split transport shim for left-half pointer movement and scroll, using RMK custom events from peripheral to central
- ZMK-style dynamic cursor and scroll scaling groups. The system layer maps `User9..User13` to XY +, XY -, scroll +, scroll -, and reset-all controls, and a local RMK vendor hook forwards those user actions to the central trackpad report path. The scaler uses the upstream x10 defaults and bounds (`10`, `2..50`) with per-axis remainders. The local RMK storage patch persists the cursor and scroll scale x10 values and restores them on boot.
- ZMK-derived cursor gating: relative cursor reports are emitted only for one-finger frames with `TP_MOVEMENT_DETECTED`
- ZMK-derived cursor scaling with remainder accumulation so small deltas are not dropped. The upstream ZMK normal-speed config uses `zip_xy_scaler 1 5`; the RMK default cursor divisor is now `5`. When layer 1 or 2 is active, a local RMK layer-state controller switches the cursor divisor to the upstream `lowspeedmode` value `15`. RMK 0.8 reports only the highest active layer to controllers, so the system tri-layer is also treated as low-speed because it is reached by holding layers 1 and 2 together.
- hardware-tested cursor orientation: X and Y are left unchanged by default. The earlier X inversion was reversed on hardware.
- a generic RMK `InputDevice` wrapper that waits for the IQS9151 active-low RDY pin, reads the ZMK-matched 28-byte coordinate frame, and emits virtual-key press/release events
- a ZMK-derived IQS9151 startup sequence before frame reads: product-number check, software reset, show-reset wait, reset ACK, Azoteq configuration block writes, resolution / ATI target / dynamic-filter overrides, ATI request, and event-mode enable. The RMK port treats show-reset timeout as non-fatal because this firmware currently has no always-visible logging path and must avoid remaining permanently silent on hardware whose RDY/show-reset timing differs from the ZMK driver.
- RDY waiting follows the upstream ZMK driver's bounded-wait model: startup/configuration and runtime reads wait for the active-low RDY level, but continue after a short timeout instead of blocking forever. This keeps the IQS9151 task alive even if RDY polarity, wiring, or sensor state is wrong.
- transient runtime coordinate-read failures do not immediately restart the full sensor initialization. The driver resets gesture state after a read failure and only falls back to full initialization after repeated consecutive failures.
- after repeated init failures, the driver enters a degraded polling mode that tries coordinate reads even though the startup sequence failed. While init/degraded polling is failing, it sends a small left/right diagnostic pointer nudge about every two seconds. Seeing that nudge means the RMK controller and HID/split reporting path are alive, and the remaining failure is likely IQS9151 I2C/product/config/RDY related.
- cursor reports and split motion events use non-blocking channel sends. This matches the upstream ZMK driver's non-blocking reporting model more closely and prevents a full HID/split queue from stopping future IQS9151 reads. If a motion send fails because the channel is full, the IQS9151 input device coalesces that motion into a pending delta and retries it on later read-loop iterations.
- continuous cursor motion is not fixed-rate throttled by default. After a motion frame is read, the driver returns to the bounded runtime RDY wait; an explicit motion interval remains available only as a tuning override.
- BLE latency is tuned as a public-firmware balance rather than an absolute minimum: the host link requests a 15 ms interval with peripheral latency 1, the split link requests a 7.5 ms interval with peripheral latency 1, and the PHY stays on 1M for broader Windows adapter compatibility.
- RMK 0.8.2 is patched locally under `vendor/rmk-0.8.2` so the composite mouse HID report map can advertise high-resolution vertical wheel and horizontal AC Pan via HID Resolution Multiplier. BLE hosts can cache the old HID report map, so remove the old pairing before validating this change over Bluetooth.
- cursor release inertia defaults to on and uses the upstream ZMK recent-history gate shape: 10 ms interval, 0.95 decay, 60 ms recent window, 35 ms stale gap, 2 minimum samples, 10 minimum average speed, and a 3 second max duration. Cursor inertia is started only from normal cursor motion releases, not tap/click/scroll/pinch gesture releases.
- two-finger scroll mode starts when centroid movement crosses the upstream threshold, with a relative-motion fallback when the IQS9151 reports two fingers and movement but the absolute finger confidence bits are temporarily unavailable. The upstream LaLaPad Gen2 normal-mode setting was `zip_scroll_scaler 1 12`; the RMK default scroll and scroll-inertia divisors are now both `12`, and layer 1, layer 2, or their system tri-layer selects the upstream `lowspeedmode` scroll divisor `40`. There is no per-report step cap before HID reporting. Active scroll deltas are smoothed with a small EMA before scaling, using rounded fixed-point output so sub-unit continuous movement still reaches the scroll remainder. Zero-delta frames reset the smoothing tail instead of continuing to scroll while the fingers are held still. Vertical scroll output is inverted for natural scrolling, while horizontal pan follows the current hardware-tested sign. Per-frame scroll amount follows finger speed because it is derived from centroid or fallback relative step distance, but stationary frames do not drain accumulated scroll remainder. Release inertia uses the upstream ZMK recent-history gate shape: 10 ms interval, 0.98 decay, 60 ms recent window, 35 ms stale gap, 1 minimum sample, 4 minimum average speed, and divisor 12 in normal mode or 40 in low-speed mode.
- tap recognition now follows the upstream driver's per-finger state model more closely: one-finger, two-finger, and three-finger sessions keep separate tap candidates, previous-frame coordinates, finger-count history, tap re-entry windows, and two-finger release-pending suppression.
- deferred tap-drag press-hold follows the upstream driver's timing model: one-finger taps hold button 1 for a 160 ms re-entry window, while two- and three-finger taps hold buttons 2 and 3 for 200 ms. A second touch inside that window keeps the button held for drag, and a second tap releases the hold before emitting a click.
- tap movement tracking follows the upstream driver's coordinate-validity checks and previous-frame fallback: finger coordinates are used only when the corresponding confidence bit is set and the coordinate is not `0xffff`; otherwise the last valid coordinate is retained while the touch remains active.
- IQS9151 built-in gesture bits are parsed for diagnostics but are not used as direct clicks. Tap clicks are emitted from the ZMK-style coordinate/movement recognizer, because direct use of the hardware gesture bits produced false taps during cursor motion on the tested hardware.
- deferred trackpad holds use direct HID mouse-button state so pointer reports from either side carry the active per-side button state while dragging/selecting.
- right-half cursor reports and left-half split cursor events are emitted directly from the IQS9151 read loop, so pointer traffic no longer round-trips through the RMK controller event loop before the next sensor frame is read
- RMK event/controller/report channel sizes are raised from the defaults to give split trackpad bursts more buffer headroom
- optional axis transform settings for hardware tuning
- central/peripheral controller adapters that instantiate the right and left IQS9151 devices on `TWISPI0`, `P0_04` SDA, `P0_05` SCL, and `P1_11` RDY
- upstream BLE-related board settings that have RMK equivalents are represented in `keyboard.toml`: BLE TX power is set to +8 dBm, charge-state input is `P0_17` active-low, and charge LED output is `P0_10` active-low.
- the upstream RGBLED widget is approximated by a local RMK polling controller on the XIAO BLE RGB pins: red `P1_03`, green `P1_05`, and blue `P1_07`, all active-low. It blinks battery level using the upstream LaLaPad threshold override (`high = 30`) plus the zmk-rgbled-widget defaults for low (`20`) and critical (`10`), blinks BLE/split connection state, and shows central-side layer changes as the upstream cyan blink sequence.

Likely next steps:

- test the I2C path on hardware and confirm the IQS9151 product number on both halves
- tune per-side cursor axis inversion/swap, pointer speed divisor, and gesture thresholds
- verify the RGB LED color polarity and battery events on real XIAO BLE hardware

## Behavior Differences

The base keymap is translated from `config/lalapadgen2.keymap`, but some ZMK-specific behaviors are approximated:

- ZMK conditional layer `1 + 2 => 3` is mapped to RMK tri-layer. RMK layers
  `0..3` now mirror the upstream Default, Secondary, Tertiary, and System
  layers.
- ZMK Bluetooth controls are mapped to RMK user keys for four BLE profiles:
  `User0..User3` select profiles, `User4`/`User5` move next/previous, `User6`
  clears the current profile, `User7` toggles output, and the local RMK patch
  maps `User8` to a ZMK-style `BT_CLR_ALL` all-profile bond clear. Holding
  `User8` still keeps RMK's existing split peer clearing path available.
- The upstream base layer's `&mo 1` and `&mo 2` thumb keys are represented as
  RMK thumb layer-taps: `LT(1, Space, FAST_LAYER)` and
  `LT(2, Enter, FAST_LAYER)`. The previous RMK-only Mac layer and semicolon
  fork were removed to match the official ZMK layer shape, but the Space and
  Enter thumb keys keep the established RMK tap/hold behavior. RMK flow-tap is
  intentionally disabled because it is global in RMK 0.8.2 and otherwise forces
  thumb layer-tap keys to resolve as taps during normal typing streaks. The
  thumb layer-tap profile uses hold-on-other-press so it behaves as a reliable
  layer modifier when chorded.
- Base combos carry the committed ZMK reference bindings `Q+W => Escape` and
  `A+S => Tab`. RMK also keeps the target-side Kana/Eisu combo deltas
  `J+K => Language1` and `D+F => Language2` so the host-side keymap preserves
  the established language switching shortcuts even when the checked-out ZMK
  source has not published those combos.
  The RMK 0.8.2 combo path used here does not expose ZMK-style
  `require-prior-idle-ms`, so the firmware keeps the upstream ZMK default 50 ms
  combo timeout.
- ZMK dynamic trackpad sensitivity controls are mapped to `User9..User13` and handled by the local RMK vendor hook described above. Scale changes are persisted in RMK storage as a LaLaPad-specific record. When `clear_storage` or a reset firmware erases storage, no scale record is restored and the compiled `10/10` cursor/scroll defaults remain active until the user changes them again.
- ZMK trackpad virtual positions `52..67` are represented in the RMK keymap as rows `5..6`, and IQS9151 runtime instances are wired into the central and peripheral firmware entrypoints.
- ZMK's RGBLED widget is represented by a local RMK controller rather than the ZMK module. Battery, connection, and central layer-change indications are preserved, but on-demand `&ind_bat` / `&ind_con` keymap behaviors are not exposed as RMK keycodes.
- RMK storage is pinned to `0x000E0000` for 8 nRF flash sectors. The default
  nRF BLE storage address in RMK 0.8 is `0x00060000`, which overlaps this
  firmware's central image after trackpad and split features are linked.
  `tools/check_flash_layout.py` is wired into `cargo make uf2` to reject future
  application/storage overlaps before firmware is flashed.
- The upstream ZMK config enables sleep with a one-hour idle-sleep timeout.
  RMK's BLE split central sleep timeout is kept explicitly disabled
  (`split_central_sleep_timeout_seconds = 0`) until right-half direct pointer
  reports are wired into RMK's sleep activity signal. Enabling the timeout
  before that could let the central enter low-power connection parameters while
  the user is only moving the right trackpad.
- The current recognizer and pointer path support axis inversion/swap settings, but the actual left/right hardware orientation, pointer speed, and thresholds still need tuning after hardware testing.
- ZMK Studio is approximated by Vial support in this RMK project.

## Porting Coverage Gate

`tools/porting_coverage_manifest.toml` is the machine-readable migration
contract derived from the upstream ZMK keymap plus documented RMK-specific
deltas. It covers the exact RMK keymap array shape, all configured RMK keymap
cells, the exact RMK combo inventory, tri-layer and tap-hold behavior settings,
and golden thumb-layer scenarios for Space, Enter, and the system tri-layer. It
derives each scenario hold layer from the `LT(...)`/`MO(...)` action and checks
that it matches the manifest's `activates_layer`, so the scenario metadata
cannot drift together with the expected output. The Vial position checks also
run against `keyboard.toml` bounds and all non-empty firmware key positions
before any ZMK source checkout is required. It also covers source-backed split matrix pins,
right-central/left-peripheral orientation, BLE TX power, charge pins, RGB pins,
IQS9151 I2C/IRQ pins, trackpad scaling constants, the RMK-side IQS9151
register-address inventory, upstream IQS9151 register and bit-flag porting
classifications, product/register address values, reset/gesture bits,
IQS9151 feature-enable flags, dynamic-scale bounds, initialization byte-array
checksums, and selected ZMK driver thresholds.
It also pins the RMK keyboard device identity, Vial unlock-key chord, internal
combo capacity, combo length, debounce time, and event/controller/report channel
sizes. The channel sizes were raised for split trackpad burst headroom, so a
later RMK configuration cleanup cannot silently fall back to the smaller default
buffers.
For source constants that are intentionally marked `ported_by_behavior` or
`ported_by_config_image`, the manifest must also point at concrete passing
coverage results. A reason string alone is not enough for those classifications
to contribute to a 100% software migration rate, and each evidence reference is
part of the coverage result inventory so changing it requires an intentional
baseline update. Behavior-based classifications should reference recognizer or
report-path unit-test coverage, while config-image classifications should
reference both byte-array parity and the sensor write path. Unit-test evidence
entries must point at active, non-ignored `#[test]` functions so an ignored
test cannot keep a migration classification green.
Source-backed checks also verify the ZMK global Kconfig flags,
left/right IQS9151 Kconfig parity, trackpad listener split routing, and
tap/gesture timing constants that are mirrored into RMK. The gate also verifies
that every active ZMK Kconfig key in the configured source files is classified
by the migration contract, compares the Vial matrix positions against the
upstream ZMK layout JSON, verifies that ZMK's `default_transform` matrix map
matches that same layout order, confirms the active ZMK physical layout points
at that transform, checks the ZMK physical-layout key attributes, checks the ZMK
repo-level and config source-file inventories, checks the upstream ZMK workflow
and build-matrix files, checks ZMK layout JSON metadata and per-key
coordinates, checks the ZMK keymap layer and behavior inventories, checks ZMK
behavior-node and combo-node
property inventories, checks ZMK `#include` dependencies, checks the ZMK
active and disabled Kconfig line order and values, checks the ZMK `Kconfig.*`
shield/default entries, checks DTS aliases and DTS root/overlay node inventories,
checks the `west.yml` ZMK module inventory,
checks the ZMK trackpad virtual-position `#define` inventory, checks the ZMK
`INPUT_BTN_*` to virtual-position binding inventory, checks the ZMK
trackpad-to-position behavior and input-processor properties, checks the ZMK
split-input container properties, checks the ZMK trackpad listener device and
normal/low-speed input-processor chains, checks
selected ZMK DTS properties for split input, dynamic scaling, right-half column
offset, overlay trackpad routing, matrix transform, physical-layout, RGB LED,
I2C, and charge-indicator nodes, checks source GPIO pin flags and their RMK
target polarity mirrors, checks the ZMK `*.dtsi` / `*.overlay` status-node
inventory, and checks the RMK custom keycode order and labels used for the ZMK
Bluetooth and trackpad scale actions.
These high-risk ZMK source checks are represented as structured inventories
rather than regex-only checks.
The thumb tap/hold layer-resolution scenarios are also cross-checked against
the ZMK source keymap after documented RMK deltas are applied, so scenario
expectations must stay source-backed instead of becoming RMK-only assertions.
RMK runtime tests cover the thumb-layer order both ways: Space then Enter and
Enter then Space must both activate the ZMK `1 + 2 => 3` system layer without
leaking ordinary keyboard reports from system actions.
The same golden scenarios are registered as RMK host-runtime tests, including
the system tri-layer `User7`, `User0`, and `Reboot` positions. Those non-HID
actions are checked by asserting that the sequence does not fall through to a
keyboard HID report from a lower layer. The runtime inventory gate scopes its
needles to the declared Rust test function, preventing an expected action in one
test from satisfying another scenario's coverage entry.
Space and Enter also have timeout-driven RMK runtime scenarios: each thumb key
is held past the documented `FAST_LAYER` `hold_timeout` before pressing `Y`, so
both the hold-on-other-press path and the long-hold path must select the
ZMK-derived layer output.
It also parses the hand-written RMK runtime-test `lalapad_keymap()` fixture and
checks its layer/row/column shape against the shipped `keyboard.toml`, and then
compares every scenario-relevant cell by layer, row, and column against that
same source of truth. A runtime test cannot keep passing against a stale
fixture after the real firmware keymap changes. A companion gate derives the
required mirror coordinates from the runtime scenario inventory and the
scenario layer-resolution manifest, so adding a new runtime scenario without
covering its coordinates also fails the migration denominator. The mirror
coverage gate also scans the runtime fixture itself and requires every non-`No`
cell, including transparent fallthrough cells, to be mirrored against
`keyboard.toml`. The mirror position inventory is also checked for duplicate
and out-of-bounds coordinates so the migration denominator cannot be inflated
or weakened by stale coordinate entries.
The gate additionally resolves every position on layer 1, layer 2, and the
system tri-layer against the ZMK source keymap to catch transparent-key
fallthrough drift beyond the hand-written representative scenarios.
ZMK `mt2` hold-tap timing values are mirrored against the RMK Morse timing
settings to catch timing drift on either side of the migration contract.

When the upstream checkout from `metadata.source_repo_hint` is available, the
gate also parses `config/lalapadgen2.keymap` directly and checks the manifest
against the raw source keymap cells, the explicitly documented RMK deltas,
documented target-side combo deltas, combo definitions, hold-tap timing
settings, conditional layer rule, shield
`*.dtsi` / `*.overlay` pins, ZMK `*.conf` values, and RMK Rust constants. Use
`--zmk-keymap PATH --require-zmk-source` to make that source-backed check
mandatory in a different checkout layout.
Source-backed monitoring gates also use `--require-zmk-source-commit`, so the
resolved source checkout must match the manifest-pinned
`metadata.source_commit` before a complete migration claim can pass.

Run:

```sh
cargo make porting-coverage
cargo make migration-status
cargo make migration-status-release-ready
cargo make host-parity-tests
cargo make rmk-behavior-tests
```

The firmware GitHub Actions workflow checks out `e-sp9/zmk-config-LalaPadGen2`
at the manifest-pinned `metadata.source_commit`, parses `vial.json`,
RMK/Cargo/manifest TOML, and flash layout, runs this source-backed
complete-porting gate, runs the host-side parity test suite and vendored RMK
behavior regression suite as part of the same local porting gate,
and keeps a standalone vendored RMK behavior regression step visible before
building release binaries. These local host parity, ZMK scenario, and RMK
behavior tasks run with `RUSTFLAGS=-Dwarnings`, and the build-task coverage
manifest fails if that warning-as-error setting is removed.
The gate
must report `100.00%` coverage and `100.00%` implementation status against the
committed upstream checkout used by CI. The
denominator can grow when upstream source files add classified behavior, so the
exact count should be read from the command output. This is a static,
source-backed, and scenario-level RMK
configuration coverage metric, not a claim that hardware-only IQS9151, BLE,
storage, or Vial runtime paths have been exhaustively exercised on real devices.
The Cargo dependency checks also keep RMK default features enabled so the
default storage/Vial support from the selected RMK release is not accidentally
disabled, while the RMK config gate pins `[host].vial_enabled = true` and the
Vial unlock chord. They also pin the split firmware entrypoints to the expected
`central` and `peripheral` binaries. The same dependency gate walks the
vendored RMK feature graph and verifies that the default feature closure still
reaches `storage`, `vial`, `vial_lock`, and `host`, that the RMK 0.8
`passkey_entry` Cargo feature still reaches BLE/storage support, and that the
enabled BLE features still resolve to `storage`. RMK main documents
`[ble].passkey_entry`, but the selected RMK 0.8.2 config schema does not accept
that key; this firmware therefore pins passkey entry through Cargo features for
the 0.8.2 line.
The build-task checks pin the cargo-make release path as well: release builds
must run the flash-layout config guard, objcopy the central and peripheral
ELFs into matching HEX files, convert both halves to nRF52840 UF2 artifacts,
run the generated-UF2 flash-layout guard, and expose a firmware artifact
manifest command for recording file sizes and SHA256 hashes.
Release-workflow checks keep the CI-generated DFU zip names, GitHub Release
asset list, generated artifact hash manifest, Pages bundling workflow, and
web-flasher bundled URLs aligned so a renamed artifact cannot pass software
migration while breaking browser flashing.
They also pin the firmware workflow's source-backed migration gate, host
parity tests, RMK behavior tests, and release build command in the migration
coverage denominator so CI cannot silently stop exercising the ZMK-derived RMK
runtime scenarios before firmware generation.
The same command also prints an explicit IQS9151 symbol porting status summary:
`ported`, `ported_by_behavior`, and `ported_by_config_image` count as
implemented, while `not_ported` entries are the remaining software-porting
items to burn down toward a true 100% implementation status. CI uses
`--require-porting-complete`, so any future non-implemented status is a release
blocker.
The text and JSON reports also include a `by_kind` breakdown of the same
coverage results, so a regression can be traced to RMK keymap/config checks,
ZMK source inventories, Kconfig/DTS mirrors, Cargo dependency resolution, Rust
constants, IQS9151 byte arrays, local RMK composite mouse and dynamic-scale
storage patch invariants, Vial identity and custom-key semantics, or firmware
code-needle checks instead of treating the total percentage as a black box.
They also include the ZMK keymap path, source availability, Git repository
path, Git commit, dirty state, and dirty path list so the migration percentage
can be traced back to the exact upstream checkout used as the source contract.
The combined release-readiness gate requires that source checkout to be clean,
so a dirty ZMK reference cannot be hidden behind a 100% software coverage
number.
The Vial checks also pin a normalized KLE geometry signature, so row grouping,
spacing directives, and half-size thumb/trackpad controls cannot drift while
the same matrix coordinates remain present.
GPIO flag mirror checks specifically tie active-low, pull-up, and open-drain
ZMK DTS flags to the RMK TOML fields and Rust constructor paths that implement
the same electrical behavior.
Trackpad virtual-button checks compare the IQS9151 runtime input button code
mapping, left/right button-position arrays, layer-0 key actions, and
Vial-exposed positions together so a gesture cannot silently move to a different
RMK key.
The split checks also verify the RMK central/peripheral matrix footprint,
row/column pin order, absence of incompatible top-level matrix or serial
configuration, and central/peripheral controller topology. The controller
topology check inspects each `#[controller(...)]` function body so the right
central trackpad must own HID reporting and the left peripheral trackpad must
use split-event transport in the intended entrypoint, not merely somewhere in
the same Rust file.
`tools/porting_coverage_baseline.toml` records the current
overall denominator, result-id inventory hash, and per-kind denominator. Use
`--coverage-baseline` in CI and release checks so a removed or swapped check
fails explicitly instead of producing a smaller `100.00%`.
`tools/hardware_validation_baseline.toml` does the same for the real-hardware
validation manifest: it freezes the 12-check inventory hash plus area, side,
and default status totals before evidence overlays are applied. Use
`--hardware-baseline` with the normal CI and release dashboard gates so
hardware-only requirements cannot be removed, renamed, or reclassified to
manufacture a smaller final-validation denominator.
It is intended to prevent regressions like a visible `LT(...)` binding whose
tap-hold behavior is changed by RMK's global flow-tap setting.
The Vial/RMK layout gate also pins the Space and Enter thumb positions as
visible Vial keys whose compiled default actions are `LT(1, Space, FAST_LAYER)`
and `LT(2, Enter, FAST_LAYER)`, including the tap key, hold layer, and Morse
profile name. This makes a later move back to `LCtrl`, `MO(...)`, or a hidden
Vial position visible as a software migration failure before flashing.
Runtime scenario coverage also requires each registered RMK scenario function
to remain an active `#[test]` and not be `#[ignore]`, and includes LaLaPad's
Q+W/Escape, A+S/Tab, J+K/Language1, and D+F/Language2 combo outputs as RMK
host-runtime HID checks rather than only static combo inventory entries.

`tools/migration_status.py` is the combined dashboard for release review. It
runs the same source-backed software checks, the coverage-denominator baseline,
the hardware validation baseline, and the hardware validation tracker in one
report. The CI gate uses `--require-software-complete` and
`--require-hardware-classified`, which means software migration must stay at
100% while hardware-only checks are allowed to remain unvalidated but cannot
become malformed or untracked. The Markdown report includes hardware progress
by area, by side, and by remaining check, including required observation terms,
required artifact types, and retained `artifact_paths`, so the path from 0/12
to 12/12 stays visible in GitHub Actions summaries. A
true final hardware claim should add `--require-hardware-validated
--require-firmware-ref <tag-or-commit>`.
The full release-validation command should use the cargo-make entrypoint so
the RMK runtime scenario suite runs before the migration dashboard is
evaluated:

```sh
HARDWARE_EVIDENCE=path/to/evidence.toml FIRMWARE_REF=tag-or-commit FIRMWARE_ARTIFACT_MANIFEST=firmware-artifacts.local.json cargo make migration-status-final
```

The final gate adds `--require-zmk-clean-source` and
`--require-zmk-source-commit`, so the resolved ZMK source keymap must be
available inside a readable Git repository with no uncommitted changes, and
that repository must be at the manifest-pinned `metadata.source_commit`, before
a complete validation claim can pass.
It also requires each validated hardware evidence entry to include at least one
existing `artifact_paths` file. Relative paths are resolved from
`EVIDENCE_ARTIFACT_ROOT` when set, otherwise from the current directory, so the
pass condition is tied to actual captured videos, logs, photos, screenshots, or
scope traces rather than text notes alone. The file type must also match the
check's `evidence_artifacts`; for example, a `video` requirement cannot be
satisfied by only attaching a `.log` file. Each retained file must be named by
path or basename in `artifact_or_notes`, and separate required artifact types
must have separate retained files.

For evidence captured from the current clean commit, the current-ref variant
derives the exact tag or short commit automatically and refuses tracked or
untracked non-ignored changes. It also runs
`cargo make firmware-artifact-manifest-current` first so the final gate checks
the current UF2 artifact hashes rather than a stale local manifest:

```sh
HARDWARE_EVIDENCE=hardware-validation-evidence.local.toml cargo make migration-status-final-current
```

For the same Markdown dashboard that CI appends to the GitHub Actions summary,
run:

```sh
cargo make migration-status-report
```

The `cargo make migration-status*` entrypoints run
`cargo make rmk-zmk-scenario-tests`, `cargo make host-parity-tests`, and
`cargo make rmk-behavior-tests` before calculating the reported migration
percentage. This keeps the monitor tied to the vendored RMK runtime scenarios
for Space, Enter, and the system tri-layer plus the broader RMK tap/hold,
layer, combo, macro, and one-shot regression suite instead of relying only on
static keymap/config coverage.
Use `cargo make migration-status-release-ready` when the software-complete,
hardware-classified, clean ZMK source, and release-readiness dashboard line
must fail the command instead of only appearing in the report.
`cargo make rmk-behavior-tests` uses `--tests`, so it intentionally re-runs the
LaLaPad scenario test binary while exercising the full vendored RMK host suite.

To re-run the vendored RMK host regression suite locally, including tap/hold,
layer, combo, macro, and one-shot behavior, run:

```sh
cargo make rmk-behavior-tests
```

When reviewing partial hardware evidence before the final all-validated gate,
include the local evidence overlay and flashed firmware reference:

```sh
HARDWARE_EVIDENCE=path/to/evidence.toml FIRMWARE_REF=tag-or-commit cargo make migration-status-report
```

## Real-Hardware Validation Gate

`tools/hardware_validation_manifest.toml` is the separate tracker for evidence
that cannot be produced by source parsing or host-side tests. It covers the
IQS9151 I2C identity and RDY signal on both halves, right and left trackpad
runtime behavior, cross-side drag behavior, BLE split pairing/reconnect, Vial
thumb layer-tap behavior, RGB/battery indicators, charge pins, and reset/reflash
behavior.

Run:

```sh
python3 tools/hardware_validation.py --require-classified
python3 tools/hardware_validation.py --hardware-baseline tools/hardware_validation_baseline.toml --require-classified
python3 tools/hardware_validation.py --markdown
python3 tools/hardware_validation.py --checklist
python3 tools/hardware_validation.py --evidence-template
cargo make hardware-validation-evidence-template-current
cargo make hardware-validation-session-current
cargo make rmk-zmk-scenario-tests
python3 tools/firmware_artifact_manifest.py --require-uf2 --require-reset-uf2 > firmware-artifacts.local.json
cargo make firmware-artifact-manifest-current
python3 tools/hardware_validation.py --evidence-template --firmware-ref-template <tag-or-commit>
python3 tools/hardware_validation.py --evidence path/to/evidence.toml --markdown
python3 tools/hardware_validation.py --evidence path/to/evidence.toml --require-validated --require-firmware-ref <tag-or-commit>
```

The command prints the real-hardware validation rate and remaining evidence
needed. JSON, text, and Markdown output include area-level and side-level
progress plus each check's required observation terms so trackpad, split, Vial,
status LED, battery, and storage gaps can be tracked independently. The
`--checklist` output turns the same manifest into a compact bench checklist for
collecting `artifact_or_notes` observations before filling an evidence overlay.
Each checklist item includes the overlay id, validation fields, required
observations, and artifact-note guidance so bench notes can be copied into the
TOML overlay without changing the hardware validation denominator.
CI uses
`--require-classified` only, which means every
hardware-only item must have a valid status, evidence description, and `source`
link to an existing Markdown heading. CI intentionally does not use
`--require-validated`; changing a check to `validated` requires actual device
evidence from the checklist, not just a green software build. The Markdown mode
emits the same tracker as tables for release notes, PR review, and the GitHub
Actions step summary. Hardware evidence can also be recorded in a separate
overlay file using the format in
`tools/hardware_validation_evidence.example.toml`; each evidence entry updates
one manifest check by id and must provide `validated_at`, `tester`,
`firmware_ref`, and `artifact_or_notes` before it can count as `validated`.
Evidence entries that declare `status = "validated"` but fail these checks are
reported as `validated_invalid` in the remaining table, so they remain visible
as work to fix and do not inflate the validated count.
Each manifest check must also declare non-empty `evidence_needles`, so
`--require-hardware-classified` catches hardware checks that lack concrete
observation terms before any release claim is made. Side-specific checks must
include their side in those observations, so a right-half I2C log cannot be
reused as left-half evidence.
`validated_at` must be a real `YYYY-MM-DD` date that is not in the future.
`firmware_ref` is an exact string match against the release tag, commit hash, or
other immutable identifier for the central/peripheral firmware pair that was
flashed; placeholders and moving refs such as `main`, `latest`, and `HEAD` are
rejected. `artifact_or_notes` must describe concrete observed evidence such as
a photo, log, probe reading, Vial observation, or similar check-specific note,
and must mention the check-specific observation terms declared by
`evidence_needles` in the hardware validation manifest. Required observation
words alone, such as only `right cursor tap vertical scroll horizontal scroll`,
do not count as concrete evidence unless the note also points to a log, photo,
probe/scope/multimeter reading, serial/I2C output, Vial observation, or similar
bench artifact.
`--require-firmware-ref` only rejects stale validated evidence; combine it with
`--require-validated` when all checks must be proven for that exact firmware.
The combined migration gate requires `--firmware-artifact-manifest` whenever
`--require-hardware-validated` is used. Whenever a firmware artifact manifest
is supplied, every validated hardware evidence note must mention that
manifest's `pair_sha256`, so both partial hardware dashboards and final
hardware claims stay tied to the exact generated normal/reset UF2 artifact hashes. The
dashboard also re-reads the artifact files under `--artifact-root` (default:
the current directory) and checks their recorded size and SHA256, so a stale
`firmware-artifacts.local.json` cannot silently validate a different local UF2
set. Plain report commands still render these as dashboard errors; add
`--require-hardware-classified` or use the final cargo-make gates when stale or
incomplete artifact evidence should make the command fail.
Use `--evidence-template > hardware-validation-evidence.local.toml` to generate
a complete local overlay for all current hardware checks, or add
`--firmware-ref-template <tag-or-commit>` to pre-fill the flashed firmware
reference before testing. Prefer `cargo make
hardware-validation-evidence-template-current` when validating a clean local
build; it pre-fills the current exact tag or short commit and fails if the
working tree has tracked or untracked non-ignored changes.
After building the UF2 files that will be flashed, use
`python3 tools/firmware_artifact_manifest.py --require-uf2 --require-reset-uf2
> firmware-artifacts.local.json` to preserve the exact normal and
storage-clear file sizes and SHA256 hashes alongside the hardware evidence
overlay. Prefer `cargo make firmware-artifact-manifest-current` for a clean
local build; it creates `firmware-artifacts.local.json`, or
`FIRMWARE_ARTIFACT_MANIFEST` when set, with the current exact tag or short
commit as `firmware_ref` and refuses tracked or untracked non-ignored changes.
When any migration-status command is run with `--firmware-artifact-manifest`,
each validated hardware evidence note must also mention that manifest's
`pair_sha256`, tying the bench observation to the exact normal/reset UF2 set
rather than only to a moving file name or host-side state. `--evidence-template`
and `--checklist` accept `--artifact-pair-sha256-template <sha256>` to seed
every evidence note with that hash before bench observations are added. Add
`--firmware-artifact-manifest-template firmware-artifacts.local.json` to make
the generated bench packet print the exact firmware paths, sides, roles, sizes,
and SHA256 hashes that the evidence is tied to.
Use `cargo make hardware-validation-session-current` to prepare a complete
current-ref bench packet in one step: RMK ZMK-derived runtime scenario results,
project host parity and RMK behavior regression results, rebuilt UF2 artifact
hashes, a firmware-ref and pair-SHA-prefilled evidence overlay, a hardware
checklist that repeats the same firmware ref, pair SHA, and per-artifact
hashes, and a local Markdown migration status report.
