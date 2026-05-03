# Trackpad Hardware Check Plan

Use this checklist when the IQS9151 trackpad does not move the cursor or when long continuous touches still stall.

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

The current diagnostic firmware intentionally sends a tiny left/right cursor nudge about every two seconds while IQS9151 initialization or degraded coordinate polling is failing. Use that as the first split:

- Diagnostic nudge appears, but touch does not work: RMK controller execution and HID reporting are alive; focus on IQS9151 I2C/product/config/RDY.
- No diagnostic nudge and no touch response: focus on whether the custom trackpad controller task is running and whether mouse HID reports are reaching the host.
- Diagnostic nudge stops after touching starts working: initialization recovered and the nudge was only reporting the earlier failure state.

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
