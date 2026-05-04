# Community Announcement Draft

Use this as a base when announcing the firmware to the LaLaPad / keyboard
community.

## Short Japanese Version

LaLaPad Gen2 向けの RMK 版ファームウェアを公開しました。

- 対象: LaLaPad Gen2 + Seeed Studio XIAO nRF52840 / XIAO BLE
- 構成: BLE split、右手側 central、左手側 peripheral
- 対応済み: キーマトリクス、5-way switch、Vial、BLE profile 操作、コンボ、IQS9151 トラックパッド、カーソル、スクロール、pinch、tap/tap-drag、3本指ジェスチャ、充電LED、RGB LEDステータス表示
- 書き込み: GitHub Release の UF2、または同梱 Web Flasher
- 注意: 公式ファームウェアではなくコミュニティ版です。書き込み前に既存のBLEペアリングを削除し、左右に対応するファームウェアを同じリリースから書き込んでください。

既知の未検証点は、実機ごとのIQS9151軸/速度/しきい値、バッテリー表示、RGB LED極性です。動作報告やチューニング値のPRを歓迎します。

## Longer Japanese Version

LaLaPad Gen2 の ZMK ファームウェアで使われていた機能を、RMK ベースで再構築したコミュニティ版ファームウェアを公開しました。

主な内容:

- Seeed Studio XIAO nRF52840 / XIAO BLE 対応
- BLE split keyboard
- 右手側 central / 左手側 peripheral
- Vial 対応
- ZMK 由来のコンボ、tri-layer、Bluetooth操作、トラックパッド仮想キー位置
- IQS9151 のカーソル、スクロール、pinch、tap、tap-drag、3本指スワイプ
- 充電状態ピン、充電LED、RGB LEDによるステータス表示
- GitHub Actions による UF2 / DFU zip 生成
- Web Flasher 同梱

注意点:

- これは公式ファームウェアではなくコミュニティ版です。
- 書き込みは自己責任でお願いします。
- 右手側には central、左手側には peripheral のファームウェアを書き込んでください。
- BLE HID descriptor の変更があるため、動作確認前にホスト側の古いペアリングを削除して再ペアリングしてください。
- IQS9151 の軸、速度、しきい値、バッテリー表示は実機差があり得るため、動作報告を歓迎します。

## Links To Include

- Repository: `https://github.com/e-sp9/lalapad-gen2-rmk`
- Releases: `https://github.com/e-sp9/lalapad-gen2-rmk/releases`
- Web Flasher: GitHub Pages URL after Pages is enabled for the repository
- Porting notes: `docs/PORTING.md`
- Hardware check plan: `docs/TRACKPAD_HARDWARE_CHECK.md`

## Announcement Checklist

- Repository visibility is public.
- License files are present.
- Latest release assets are available.
- Pages deployment is enabled and the web flasher URL is confirmed.
- Known limitations in the announcement match `README.md`.
