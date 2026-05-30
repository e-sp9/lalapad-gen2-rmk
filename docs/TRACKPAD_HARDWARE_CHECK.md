# Trackpad Hardware Check Plan

Use this checklist when the IQS9151 trackpad does not move the cursor or when long continuous touches still stall.

The machine-readable hardware validation tracker is
`tools/hardware_validation_manifest.toml`. After running these checks, update
the relevant check status and evidence there so the software porting coverage
gate and real-device validation rate stay separate.

## Firmware Baseline

1. Flash the newest UF2 artifacts to both halves:
   - `firmware/normal/lalapad-gen2-rmk-central.uf2` to the right half.
   - `firmware/normal/lalapad-gen2-rmk-peripheral.uf2` to the left half.
2. Reconnect the right half first, then the left half.
3. Remove the old Bluetooth pairing on the host and pair again before testing over BLE. The scroll firmware changes the HID report map, and hosts can keep using the cached old map until re-pairing.
4. Confirm ordinary key matrix input works on both halves before testing the trackpads.
5. Test the right trackpad alone, then the left trackpad alone, then both trackpads together.
6. Test two-finger vertical scroll, then two-finger horizontal scroll. Confirm that scroll does not also move the cursor or emit right-click taps during continuous movement. Flick once and release to confirm inertia continues briefly, then stops when a finger touches the pad again.
7. Hold one finger still on either trackpad until left mouse button hold starts, then move the other trackpad. Confirm the host performs drag/select and releases the selection when the held finger is lifted.

## BLE Split Pairing And Reconnect Check

Use this check to prove that the right central and left peripheral both joined
the split link after a host BLE bond reset. A successful host connection to the
right central half alone is not enough, because the left half can remain
disconnected while the keyboard still appears paired.

1. Flash the current normal central UF2 to the right half.
2. Flash the current normal peripheral UF2 to the left half.
3. Remove the old host BLE pairing.
4. Power or reset the right central half first, then the left peripheral half.
5. Pair the keyboard over BLE again.
6. In a host key-event viewer, press left-side `Q` and `A`.
7. Press right-side `Y` and `H`.
8. Power-cycle or reset both halves, reconnect right first and left second, then
   repeat the same `Q`, `A`, `Y`, and `H` key checks.

Hardware evidence for this check must name the right central, left peripheral,
BLE re-pair, reconnect order, left `Q`, left `A`, right `Y`, and right `H`
observations.

## Trackpad Cursor Tap Scroll Check

Use this check for each half after the firmware baseline setup. The right half
reports directly from the central firmware. The left half must prove the same
behavior through the split custom-event path.

1. Move one finger and confirm cursor motion on the tested half.
2. Tap with one finger and confirm a left-click tap.
3. Perform two-finger vertical scroll.
4. Perform two-finger horizontal scroll.
5. While continuously scrolling, confirm there is no cursor motion and no
   right-click tap.
6. Flick once and release. Confirm scroll inertia continues briefly.
7. Touch the pad again and confirm inertia stops on touch.

Hardware evidence for this check must name the tested side, cursor, tap,
vertical scroll, horizontal scroll, no cursor during scroll, no right-click
during scroll, inertia continues, and inertia stops on touch observations. Left
trackpad evidence must also name the split path.

## Cross-Side Trackpad Drag Check

Use this check after both halves are paired and both trackpads have passed the
cursor/tap/scroll check. This proves that deferred left-button hold state from
one side is carried in host reports while the other side moves the pointer.

1. Hold one finger on the right trackpad until left-button hold starts, move the
   left trackpad, and confirm host drag/select.
2. Lift the held right-trackpad finger and confirm the host selection releases.
3. Hold one finger on the left trackpad until left-button hold starts, move the
   right trackpad, and confirm host drag/select.
4. Lift the held left-trackpad finger and confirm the host selection releases.
5. Confirm no mouse button remains stuck after both directions.

Hardware evidence for this check must name cross-side drag, right hold with
left move, left hold with right move, left-button hold, host drag/select,
release on held-finger lift, and no stuck button observations.

## RGB Status Widget Check

Use this check on the right central half. The XIAO BLE RGB LED channels are
active-low and wired as red `P1_03`, green `P1_05`, and blue `P1_07`.

1. Confirm the RGB pins are active-low: an on channel drives low, and an off
   channel drives high.
2. After firmware/widget reset, send or observe battery level 30 as the first
   battery report and confirm green.
3. After another firmware/widget reset, send or observe battery level 20 as the
   first battery report and confirm yellow.
4. Send or observe battery level 10 and confirm the critical battery report
   blinks red.
5. Confirm BLE connected blinks blue.
6. Confirm BLE advertising blinks yellow.
7. Confirm BLE disconnected/no-profile state blinks red.
8. Connect and disconnect the split side and confirm split connected blinks
   blue and split disconnected blinks red.
9. Change to a non-default central layer and confirm the layer indication blinks
   cyan.

Hardware evidence for this check must name the right half, RGB, `P1_03 red`,
`P1_05 green`, `P1_07 blue`, active-low polarity, battery 30 green after reset,
battery 20 yellow after reset, battery 10 red critical blink, BLE connected
blue, BLE advertising yellow, BLE disconnected red, split connected blue, split
disconnected red, and layer cyan observations.

## Charge Indicator Pin Check

Use this check on the right central half. RMK is configured with charge-state
input `P0_17` active-low and charge LED output `P0_10` active-low.

1. With a charge source connected, measure or log `P0_17` and confirm the
   charge-state input is low.
2. With the charge source disconnected or not charging, measure or log `P0_17`
   and confirm the charge-state input is high.
3. Observe or measure the charge LED output `P0_10` while the LED is on and
   confirm the output is low.
4. Observe or measure the charge LED output `P0_10` while the LED is off and
   confirm the output is high.

Hardware evidence for this check must name the right half, `P0_17
charge-state`, active-low polarity, USB charging low, not charging high, `P0_10
charge LED`, LED on low, and LED off high observations.

## Thumb Layer-Tap Check

Use a host key-event viewer after flashing the current firmware and re-pairing
BLE. Vial layout inspection alone is not enough, because Vial can show the
configured `LT(...)` key while host output is still affected by stale storage,
tap-hold timing, or an incorrect RMK action.

1. Open the keyboard in Vial and confirm the thumb positions show Space as
   layer 1 and Enter as layer 2.
2. Tap Space and Enter normally and confirm they still emit Space and Enter.
3. Hold Space, press Y, release Y, then release Space. Confirm the host sees
   `NumLock`, proving the Space hold path selected layer 1.
4. Hold Enter, press Y, release Y, then release Enter. Confirm the host sees
   `PageUp`, proving the Enter hold path selected layer 2.

## Storage Reset And Reflash Check

Use this check when validating that host/Vial behavior is not coming from stale
RMK storage. RMK storage persists Vial keymap changes and BLE bond data, so a
normal firmware reflash alone does not prove the compiled default keymap is what
the host is using.

1. Flash the reset/storage-clear UF2 for the right central half.
2. Flash the reset/storage-clear UF2 for the left peripheral half.
3. Flash the matching normal central UF2 to the right half.
4. Flash the matching normal peripheral UF2 to the left half.
5. Remove the old host BLE pairing, reconnect the right half first, then the
   left half, and pair again.
6. Open Vial and confirm the default keymap is visible, then repeat the thumb
   layer-tap check above.

Hardware evidence for this check must name the reset central UF2, reset
peripheral UF2, normal central UF2, normal peripheral UF2, BLE re-pair, and
Vial observation so the final validation gate cannot accidentally accept a
normal-only reflash.

The current diagnostic firmware intentionally sends a tiny left/right cursor nudge about every two seconds while IQS9151 initialization or degraded coordinate polling is failing. Use that as the first split:

- Diagnostic nudge appears, but touch does not work: RMK controller execution and HID reporting are alive; focus on IQS9151 I2C/product/config/RDY.
- No diagnostic nudge and no touch response: focus on whether the custom trackpad controller task is running and whether mouse HID reports are reaching the host.
- Diagnostic nudge stops after touching starts working: initialization recovered and the nudge was only reporting the earlier failure state.

## IQS9151 I2C Identity Check

Run this check separately on the right and left halves. The IQS9151 I2C bus is
wired to `P0_04` SDA and `P0_05` SCL on both XIAO BLE boards.

1. Confirm `P0_04` SDA and `P0_05` SCL idle high and show I2C activity after
   reset.
2. Run an I2C scan or diagnostic firmware on the tested half.
3. Confirm the IQS9151 acknowledges address `0x56`.
4. Read product-number register `0x1000`.
5. Confirm the product-number value is `0x09bc`.

Hardware evidence for this check must name the tested side, `P0_04 SDA`,
`P0_05 SCL`, I2C activity from a scan or diagnostic firmware, address `0x56`,
product-number register, register `0x1000`, and product value `0x09bc`.

## IQS9151 RDY Signal Check

Run this check separately on the right and left halves. The IQS9151 RDY / IRQ
line is XIAO D6 / nRF `P1_11` and is active-low.

1. Measure `P1_11` RDY / D6 with power on and no touch.
2. Confirm the no-touch state is high.
3. Touch or otherwise trigger the sensor.
4. Confirm the touch-event state pulses low or drives low.

Hardware evidence for this check must name the tested side, `P1_11 RDY`, D6,
active-low polarity, no-touch high, and touch-event low observations.

## Expected Wiring

The upstream ZMK shield defines the IQS9151 interrupt as:

```devicetree
irq-gpios = <&xiao_d 6 (GPIO_ACTIVE_LOW | GPIO_PULL_UP)>;
```

For XIAO BLE this maps to:

| Signal | XIAO pin | nRF pin | Expected state |
| --- | --- | --- | --- |
| SDA | D4 | P0_04 | I2C data, pulled up |
| SCL | D5 | P0_05 | I2C clock, pulled up |
| RDY / IRQ | D6 | P1_11 | active low, pulled up |
| VCC | 3V3 | 3V3 | stable 3.3 V |
| GND | GND | GND | common ground |

The firmware now treats RDY as a bounded hint, not as a hard gate. If RDY is stuck high or low, the task should keep polling instead of becoming permanently silent.

## Electrical Checks

1. With power on and no touch, RDY should normally sit high because of the pull-up.
2. During touch or sensor events, RDY should pulse or drive low.
3. SDA and SCL should idle high and show I2C activity after reset.
4. The sensor should answer at I2C address `0x56`.
5. The product-number register `0x1000` should read `0x09bc`.

If both halves are silent, check shared assumptions first: power, ground, D4/D5 I2C mapping, and the IQS9151 address. If the right half works but the left half does not, check split transport and left-half wiring. If keys work but one trackpad is silent, focus on that half's I2C/RDY wiring and solder joints.

## Firmware Symptom Map

| Symptom | Most likely area |
| --- | --- |
| No cursor and no tap on both halves | I2C address/product read, shared wiring assumption, or initialization failure |
| Right cursor works but left does not | split custom event path or left IQS9151 wiring |
| Cursor starts then eventually stops | RDY timing, HID/split queue pressure, or sensor show-reset/reinitialization |
| Cursor moves but tap fires during movement | gesture threshold/state-machine tuning |
| Tap works but cursor does not | relative-motion flags or cursor gating |

## Next Diagnostic Firmware If Needed

If the current bounded-RDY firmware still produces no trackpad response, add a temporary diagnostic build that reports:

- product-number read success/failure per half
- last I2C error stage
- RDY timeout count
- coordinate-frame read count
- show-reset count
- split custom-event send/drop count

The current user-visible firmware avoids logging dependencies, so these diagnostics should stay temporary and not replace the normal keymap behavior.
