# LaLaPad Gen2 RMK

This repository is an RMK firmware project for rebuilding LaLaPad Gen2 firmware.

Current scope:

- Seeed Studio XIAO nRF52840 / XIAO BLE target
- BLE split keyboard
- Right half as central, left half as peripheral
- 42 keyboard keys plus the two 5-way switches
- Vial definition for the physical key matrix and trackpad virtual positions
- Keymap positions for the upstream trackpad button/gesture bindings

Not yet ported:

- LaLaPad Gen2's Azoteq IQS9151 runtime input driver
- Dynamic trackpad scaling controls from the upstream ZMK firmware
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

- `firmware/normal/lalapad-gen2-rmk-central.uf2` for the right half
- `firmware/normal/lalapad-gen2-rmk-peripheral.uf2` for the left half

Reset/storage-clear UF2 files, when generated for hardware testing, are kept under `firmware/reset/`.

## Web Flasher

ブラウザから直接ファームウェアを書き込めるツールを `tools/web-flasher/` に同梱しています。
GitHub Actions で `main` への push とリリース公開のたびに GitHub Pages へデプロイされます。

- 対応ブラウザ: Chrome / Edge / Opera (WebHID + WebSerial が必要)
- 中身: Adafruit nRF52 UF2 ブートローダ (XIAO BLE 同梱) と直接話す Nordic Legacy DFU クライアント
- 左右で別ファームのため、右半分 (Central) → 左半分 (Peripheral) の順に書き込みます
- 右半分は WebHID 経由で自動で DFU モードに入れます。左半分は USB HID を持たないため、リセットボタンを素早く 2 回押して手動で DFU モードに入ってください

リリース時には、`firmware/lalapad-gen2-rmk-{central,peripheral}-dfu.zip` が GitHub Release のアセットに自動添付され、Pages 側に同梱されます。手元の zip を読み込んで書き込むこともできます (UI 内 "ローカルファイルを使う")。

## Sources Used For Porting

- RMK current documentation and `nrf52840_split` template
- `ShiniNet/LaLaPadGen2`
- `ShiniNet/zmk-config-LalaPadGen2`

See `docs/PORTING.md` for pin mapping and remaining firmware gaps.
