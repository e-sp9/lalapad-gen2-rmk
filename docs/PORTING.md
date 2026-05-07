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
- two-finger pinch is routed as a ZMK-style frame result: the upstream `INPUT_BTN_7` virtual key is pressed first, pinch wheel deltas are emitted while it is held, and the key is released on pinch end. RMK custom events keep that key/wheel ordering across both the central and split paths so pinch does not degrade into plain scroll when the wheel report reaches the host before the virtual key. The RMK port intentionally reverses the pinch wheel direction from the first hardware-test build and caps pinch output to one wheel unit per report with much lower source gain, because Ctrl+wheel zoom was too abrupt with the upstream-derived wheel gain.
- two-finger mode selection keeps ambiguous early movement pending until either centroid movement clearly dominates as scroll or distance change reaches the pinch threshold. This avoids leaking an initial plain scroll frame during a pinch gesture.
- a split transport shim for left-half pointer movement and scroll, using RMK custom events from peripheral to central
- ZMK-style dynamic cursor and scroll scaling groups. The adjust layer maps `User9..User13` to XY +, XY -, scroll +, scroll -, and reset-all controls, and a local RMK vendor hook forwards those user actions to the central trackpad report path. The scaler uses the upstream x10 defaults and bounds (`10`, `2..50`) with per-axis remainders; unlike ZMK settings-backed scalers, the current RMK port does not persist scale changes across reset.
- ZMK-derived cursor gating: relative cursor reports are emitted only for one-finger frames with `TP_MOVEMENT_DETECTED`
- ZMK-derived cursor scaling with remainder accumulation so small deltas are not dropped. The upstream ZMK config uses `zip_xy_scaler 1 5`; the current RMK default divisor is `3`, after IQS9151 initialization made divisor `1` too aggressive while divisor `5` was previously too slow before the startup sequence existed.
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
- USB and BLE pointer reports use a small transport compensation stage after dynamic scaling. USB cursor motion is reduced to 75% and USB wheel/pan output to 50% to avoid coarse wired scroll jumps, while BLE cursor motion is raised to 133% and BLE wheel/pan output to 150% to offset the slower wireless host path.
- cursor release inertia is implemented behind a runtime flag but defaults to off. When enabled, it uses the upstream ZMK recent-history gate shape: 10 ms interval, 0.95 decay, 60 ms recent window, 35 ms stale gap, 2 minimum samples, 10 minimum average speed, and a 3 second max duration. Cursor inertia is started only from normal cursor motion releases, not tap/click/scroll/pinch gesture releases.
- two-finger scroll mode starts when centroid movement crosses the ZMK-derived threshold, with a relative-motion fallback when the IQS9151 reports two fingers and movement but the absolute finger confidence bits are temporarily unavailable. The upstream LaLaPad Gen2 normal-mode setting was `zip_scroll_scaler 1 12`; after adding HID Resolution Multiplier support to the RMK composite mouse report, the RMK port uses divisor 8 with a 3-step per-report cap because each wheel/pan unit is now advertised as a high-resolution fraction. Low-speed scroll deltas at or below 48 raw units use divisor 16 to keep slow continuous movement responsive without making fast scrolls jumpy. Active scroll deltas are smoothed with a small EMA before scaling, using rounded fixed-point output so sub-unit continuous movement still reaches the scroll remainder. Zero-delta frames reset the smoothing tail instead of continuing to scroll while the fingers are held still. Vertical scroll output is inverted for natural scrolling, while horizontal pan follows the current hardware-tested sign. Per-frame scroll amount follows finger speed because it is derived from centroid or fallback relative step distance, but stationary frames do not drain accumulated scroll remainder and over-cap scroll remainder is discarded instead of being drained through later frames; this avoids a small subsequent movement causing a burst from an earlier sensor spike. Release inertia uses the upstream ZMK recent-history gate shape, but is damped for RMK HID output: 10 ms interval, 0.90 decay, 60 ms recent window, 35 ms stale gap, 2 minimum samples, 4 minimum average speed, divisor 16, and a 2-step per-report cap.
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
- the upstream RGBLED widget is approximated by a local RMK polling controller on the XIAO BLE RGB pins: red `P1_03`, green `P1_05`, and blue `P1_07`, all active-low. It blinks battery level using the upstream LaLaPad thresholds (`high = 30`, critical = `10`), blinks BLE/split connection state, and shows central-side layer changes as the upstream cyan blink sequence.

Likely next steps:

- test the I2C path on hardware and confirm the IQS9151 product number on both halves
- tune per-side cursor axis inversion/swap, pointer speed divisor, and gesture thresholds
- verify the RGB LED color polarity and battery events on real XIAO BLE hardware

## Behavior Differences

The base keymap is translated from `config/lalapadgen2.keymap`, but some ZMK-specific behaviors are approximated:

- ZMK conditional layer `1 + 2 => 3` is mapped to RMK tri-layer.
- ZMK Bluetooth controls are mapped to RMK user keys for four BLE profiles: `User0..User3` select profiles, `User4`/`User5` move next/previous, `User6` clears the current profile, `User7` toggles output, and `User8` remains available for RMK split peer clearing.
- ZMK dynamic trackpad sensitivity controls are mapped to `User9..User13` and handled by the local RMK vendor hook described above. Scale changes are runtime-only for now.
- ZMK trackpad virtual positions `52..67` are represented in the RMK keymap as rows `5..6`, and IQS9151 runtime instances are wired into the central and peripheral firmware entrypoints.
- ZMK's RGBLED widget is represented by a local RMK controller rather than the ZMK module. Battery, connection, and central layer-change indications are preserved, but on-demand `&ind_bat` / `&ind_con` keymap behaviors are not exposed as RMK keycodes.
- The current recognizer and pointer path support axis inversion/swap settings, but the actual left/right hardware orientation, pointer speed, and thresholds still need tuning after hardware testing.
- ZMK Studio is approximated by Vial support in this RMK project.
