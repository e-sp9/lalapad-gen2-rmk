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

### Web Flasher

ブラウザから直接ファームウェアを書き込めるツールを `tools/web-flasher/` に同梱しています。
GitHub Actions で `main` への push とリリース公開のたびに GitHub Pages へデプロイされます。

- 対応ブラウザ: Chrome / Edge / Opera (WebHID + WebSerial が必要)
- 中身: Adafruit nRF52 UF2 ブートローダ (XIAO BLE 同梱) と直接話す Nordic Legacy DFU クライアント
- 左右で別ファームのため、左半分 (Peripheral) と右半分 (Central) の両方に書き込みます (UI も左→右の順に並べてあります)
- 右半分は WebHID 経由で自動で DFU モードに入れます。左半分は USB HID を持たないため、リセットボタンを素早く 2 回押して手動で DFU モードに入ってください

リリース時には、`firmware/lalapad-gen2-rmk-{central,peripheral}-dfu.zip` が GitHub Release のアセットに自動添付され、Pages 側に同梱されます。手元の zip を読み込んで書き込むこともできます (UI 内 "ローカルファイルを使う")。

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

The generated files are:

- `firmware/normal/lalapad-gen2-rmk-central.uf2` for the right half
- `firmware/normal/lalapad-gen2-rmk-peripheral.uf2` for the left half

Reset/storage-clear UF2 files, when generated for hardware testing, are kept under `firmware/reset/`.

## Validation

```shell
python3 -m json.tool vial.json >/tmp/lalapad-vial-check.json
python3 -c 'import tomllib; tomllib.load(open("keyboard.toml", "rb")); tomllib.load(open("Cargo.toml", "rb")); print("toml ok")'
rmkit get-chip --keyboard-toml-path keyboard.toml
rmkit get-project-name --keyboard-toml-path keyboard.toml
cargo check --release --bin central
cargo check --release --bin peripheral
cargo build --release
```

Run `cargo make uf2 --release` when changing release artifacts or flashing behavior.

## Documentation

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
