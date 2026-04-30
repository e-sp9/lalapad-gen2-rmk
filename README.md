# LaLaPad Gen2 RMK

This repository is an RMK firmware project for rebuilding LaLaPad Gen2 firmware.

Current scope:

- Seeed Studio XIAO nRF52840 / XIAO BLE target
- BLE split keyboard
- Right half as central, left half as peripheral
- 42 keyboard keys plus the two 5-way switches
- Vial definition for the physical key matrix

Not yet ported:

- LaLaPad Gen2's Azoteq IQS9151 trackpads and gesture-to-key behavior
- LED/battery widgets from the upstream ZMK firmware

## Build

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

The generated files are:

- `lalapad-gen2-rmk-central.uf2` for the right half
- `lalapad-gen2-rmk-peripheral.uf2` for the left half

## Sources Used For Porting

- RMK current documentation and `nrf52840_split` template
- `ShiniNet/LaLaPadGen2`
- `ShiniNet/zmk-config-LalaPadGen2`

See `docs/PORTING.md` for pin mapping and remaining firmware gaps.
