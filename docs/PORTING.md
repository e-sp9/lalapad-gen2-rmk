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
- ZMK dynamic trackpad sensitivity controls are mapped to `User9..User13` and handled by the local RMK vendor hook described above. Scale changes are persisted in RMK storage as a LaLaPad-specific record.
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
also covers source-backed split matrix pins,
right-central/left-peripheral orientation, BLE TX power, charge pins, RGB pins,
IQS9151 I2C/IRQ pins, trackpad scaling constants, and selected ZMK driver
thresholds. Source-backed checks also verify the ZMK global Kconfig flags,
left/right IQS9151 Kconfig parity, trackpad listener split routing, and
tap/gesture timing constants that are mirrored into RMK. The gate also verifies
that every active ZMK Kconfig key in the configured source files is classified
by the migration contract, compares the Vial matrix positions against the
upstream ZMK layout JSON, verifies that ZMK's `default_transform` matrix map
matches that same layout order, confirms the active ZMK physical layout points
at that transform, checks the ZMK physical-layout key attributes, checks the ZMK
source-file inventory, checks the ZMK keymap layer and behavior inventories,
checks ZMK `#include` dependencies, checks the ZMK `Kconfig.*` shield/default
entries, checks the `west.yml` ZMK module inventory, checks the ZMK trackpad
virtual-position `#define` inventory, checks the ZMK `INPUT_BTN_*` to
virtual-position binding inventory, checks the ZMK trackpad listener device and
normal/low-speed input-processor chains, checks selected ZMK DTS properties for
split input, dynamic scaling, right-half column offset, and overlay trackpad
routing overrides, checks source GPIO pin flags, checks the ZMK `*.dtsi` /
`*.overlay` status-node inventory, and checks the RMK custom keycode order used
for the ZMK Bluetooth and trackpad scale actions. These high-risk ZMK source
checks are represented as structured inventories rather than regex-only checks.
The thumb tap/hold layer-resolution scenarios are also cross-checked against
the ZMK source keymap after documented RMK deltas are applied, so scenario
expectations must stay source-backed instead of becoming RMK-only assertions.

When the upstream checkout from `metadata.source_repo_hint` is available, the
gate also parses `config/lalapadgen2.keymap` directly and checks the manifest
against the raw source keymap cells, the explicitly documented RMK deltas,
documented target-side combo deltas, combo definitions, hold-tap timing
settings, conditional layer rule, shield
`*.dtsi` / `*.overlay` pins, ZMK `*.conf` values, and RMK Rust constants. Use
`--zmk-keymap PATH --require-zmk-source` to make that source-backed check
mandatory in a different checkout layout.

Run:

```sh
python3 tools/porting_coverage.py --require-zmk-source
```

The firmware GitHub Actions workflow checks out `e-sp9/zmk-config-LalaPadGen2`
and runs this source-backed gate before building release binaries. The gate
must report `100.00%` against the committed upstream checkout used by CI. The
denominator can grow when upstream source files add classified behavior, so the
exact count should be read from the command output. This is a static,
source-backed, and scenario-level RMK
configuration coverage metric, not a claim that hardware-only IQS9151, BLE,
storage, or Vial runtime paths have been exhaustively exercised on real devices.
It is intended to prevent regressions like a visible `LT(...)` binding whose
tap-hold behavior is changed by RMK's global flow-tap setting.
