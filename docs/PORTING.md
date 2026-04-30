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

## Matrix Scope

The RMK matrix currently covers rows `0..4` from the ZMK transform:

- rows `0..3`: main 42-key layout
- row `4`: left and right 5-way switches

The upstream ZMK transform also defines rows `5..6` for trackpad-generated virtual button positions. Those are not included in the RMK matrix yet because they are produced by the IQS9151 input processor path, not by GPIO scanning.

## Trackpad Gap

LaLaPad Gen2 uses `azoteq,iqs9151` in the upstream ZMK shield.

RMK's current main documentation includes a TOML-configurable Azoteq IQS5xx trackpad driver for IQS550 / IQS572 / IQS525 style devices. That is not the same device as IQS9151, so the LaLaPad Gen2 trackpad behavior should be treated as a separate porting task.

Likely next steps:

- add or adapt an RMK input device driver for IQS9151
- wire the XIAO I2C pins `P0_04`/`P0_05` and RDY pin `P1_11`
- reproduce the ZMK gesture/button mapping for virtual positions `52..67`
- run the pointing processor on the central side

## Behavior Differences

The base keymap is translated from `config/lalapadgen2.keymap`, but some ZMK-specific behaviors are approximated:

- ZMK conditional layer `1 + 2 => 3` is mapped to RMK tri-layer.
- ZMK Bluetooth controls are mapped to RMK `User0..User11` keys.
- ZMK dynamic trackpad sensitivity controls are omitted until the IQS9151 path exists.
