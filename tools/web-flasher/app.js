import { requestVialDevice, isUnlocked, jumpToBootloader } from './hid.js';
import { flash, parseDfuPackage } from './dfu.js';

// Firmware is bundled into the Pages deploy (see .github/workflows/pages.yml)
// so we can fetch it same-origin and avoid the CORS restrictions on GitHub's
// release-assets CDN.
const BUNDLED_VERSION_URL = './firmware/version.json';
const BUNDLED_ZIP_URLS = {
  central: './firmware/lalapad-gen2-rmk-central-dfu.zip',
  peripheral: './firmware/lalapad-gen2-rmk-peripheral-dfu.zip',
};

// Known VID/PIDs for serial bootloader enumeration hints (user still picks via dialog).
const BOOTLOADER_HINTS = [
  { usbVendorId: 0x239a }, // Adafruit
  { usbVendorId: 0x2886 }, // Seeed (XIAO variants)
];

// LaLaPad Gen2 RMK USB VID — used to pre-filter the WebHID picker so only the
// running keyboard shows up. The peripheral does not expose USB, so this is
// only relevant for the central.
const VIAL_HID_HINTS = [{ vendorId: 0x4c4b }];

const HALVES = ['central', 'peripheral'];
const HALF_LABEL = { central: '右 / Central', peripheral: '左 / Peripheral' };

// ---------------------------------------------------------------------------
// UI helpers
// ---------------------------------------------------------------------------

const $ = (sel) => document.querySelector(sel);

const ui = {
  btnFetch: $('#btn-fetch'),
  fetchStatus: $('#fetch-status'),
  fileCentral: $('#file-central'),
  filePeripheral: $('#file-peripheral'),
  btnResetCentral: $('#btn-reset-central'),
  resetCentralStatus: $('#reset-central-status'),
  btnFlashCentral: $('#btn-flash-central'),
  flashCentralStatus: $('#flash-central-status'),
  progressCentral: $('#progress-central'),
  btnFlashPeripheral: $('#btn-flash-peripheral'),
  flashPeripheralStatus: $('#flash-peripheral-status'),
  progressPeripheral: $('#progress-peripheral'),
  log: $('#log'),
  compatHid: $('#compat-hid'),
  compatSerial: $('#compat-serial'),
};

function log(msg) {
  const ts = new Date().toLocaleTimeString();
  ui.log.textContent += `[${ts}] ${msg}\n`;
  ui.log.scrollTop = ui.log.scrollHeight;
}

function setStatus(el, text, cls) {
  el.textContent = text;
  el.className = 'status' + (cls ? ` ${cls}` : '');
}

function setProgress(el, ratio) {
  el.style.width = `${Math.min(100, Math.max(0, ratio * 100)).toFixed(1)}%`;
}

const flashStatusEl = (half) =>
  half === 'central' ? ui.flashCentralStatus : ui.flashPeripheralStatus;
const progressEl = (half) =>
  half === 'central' ? ui.progressCentral : ui.progressPeripheral;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/** @type {{ central: Blob | null, peripheral: Blob | null }} */
const firmwareBlobs = { central: null, peripheral: null };
let firmwareLabel = '';

// ---------------------------------------------------------------------------
// Compatibility check
// ---------------------------------------------------------------------------

function checkCompat() {
  const hasHid = 'hid' in navigator;
  const hasSerial = 'serial' in navigator;
  setStatus(ui.compatHid, hasHid ? '利用可能' : '未対応', hasHid ? 'ok' : 'err');
  setStatus(ui.compatSerial, hasSerial ? '利用可能' : '未対応', hasSerial ? 'ok' : 'err');
  if (!hasHid || !hasSerial) {
    log('このブラウザは WebHID / WebSerial に対応していません。Chrome / Edge / Opera の最新版をお使いください。');
    ui.btnResetCentral.disabled = true;
    ui.btnFlashCentral.disabled = true;
    ui.btnFlashPeripheral.disabled = true;
  }
}

// ---------------------------------------------------------------------------
// Phase 1: Fetch bundled firmware
// ---------------------------------------------------------------------------

async function loadBundledFirmware() {
  setStatus(ui.fetchStatus, '取得中…');
  try {
    const versionRes = await fetch(BUNDLED_VERSION_URL, { cache: 'no-store' });
    if (!versionRes.ok) {
      throw new Error(`version.json の取得に失敗: ${versionRes.status}`);
    }
    const version = await versionRes.json();
    if (!version.tag) {
      throw new Error('まだ Release がありません。`git tag v0.1.0 && git push origin v0.1.0` などで初回リリースを作成してください');
    }

    for (const half of HALVES) {
      const url = BUNDLED_ZIP_URLS[half];
      const res = await fetch(url, { cache: 'no-store' });
      if (!res.ok) {
        throw new Error(`${half} の DFU zip の取得に失敗 (${url}): ${res.status}`);
      }
      const blob = await res.blob();
      const { firmware, initPacket } = await parseDfuPackage(blob);
      firmwareBlobs[half] = blob;
      log(`${version.tag}: ${half} (${Math.round(blob.size / 1024)} KB) を取得 — firmware=${firmware.length}B init=${initPacket.length}B`);
    }
    firmwareLabel = version.tag;
    setStatus(ui.fetchStatus, `${version.tag} を取得`, 'ok');
  } catch (e) {
    firmwareBlobs.central = null;
    firmwareBlobs.peripheral = null;
    firmwareLabel = '';
    setStatus(ui.fetchStatus, '失敗', 'err');
    log(`エラー: ${e instanceof Error ? e.message : e}`);
  }
}

async function loadFirmwareFromFile(half, file) {
  setStatus(ui.fetchStatus, `${HALF_LABEL[half]} 読み込み中…`);
  try {
    const { firmware, initPacket } = await parseDfuPackage(file);
    firmwareBlobs[half] = file;
    firmwareLabel = file.name;
    log(`ローカルファイル OK [${half}]: firmware=${firmware.length}B, init=${initPacket.length}B (${file.name})`);
    const haveBoth = firmwareBlobs.central && firmwareBlobs.peripheral;
    setStatus(ui.fetchStatus, haveBoth ? '両方読み込み済み' : `${HALF_LABEL[half]} のみ`, 'ok');
  } catch (e) {
    firmwareBlobs[half] = null;
    setStatus(ui.fetchStatus, '失敗', 'err');
    log(`エラー: ${e instanceof Error ? e.message : e}`);
  }
}

// ---------------------------------------------------------------------------
// Phase 2: Reset central to bootloader via WebHID
// ---------------------------------------------------------------------------

async function resetCentral() {
  setStatus(ui.resetCentralStatus, '接続中…');
  try {
    const device = await requestVialDevice(VIAL_HID_HINTS);
    if (!device) {
      setStatus(ui.resetCentralStatus, 'キャンセル', 'warn');
      return;
    }
    log(`HID 接続: ${device.productName ?? '(no name)'} (VID ${device.vendorId.toString(16)}, PID ${device.productId.toString(16)})`);
    const unlocked = await isUnlocked(device);
    if (!unlocked) {
      log('Vial unlock 状態が検出できませんでした。ファームの security 設定によっては手動 unlock が必要です。');
    }
    log('BootloaderJump (0x0B) を送信');
    await jumpToBootloader(device);
    // The keyboard disappears before any ACK — treat immediate send as success.
    setStatus(ui.resetCentralStatus, 'リセット送信済み', 'ok');
    log('右半分が DFU モードで再列挙されたら下のボタンから書き込みへ進んでください');
  } catch (e) {
    setStatus(ui.resetCentralStatus, '失敗', 'err');
    log(`エラー: ${e instanceof Error ? e.message : e}`);
  }
}

// ---------------------------------------------------------------------------
// Phase 3: Flash via WebSerial DFU
// ---------------------------------------------------------------------------

async function flashHalf(half) {
  const blob = firmwareBlobs[half];
  const statusEl = flashStatusEl(half);
  const bar = progressEl(half);
  if (!blob) {
    log(`先に手順 1 で ${HALF_LABEL[half]} のファームウェアを取得してください`);
    return;
  }
  setStatus(statusEl, 'ポート選択中…');
  try {
    if (!('serial' in navigator)) throw new Error('WebSerial unavailable');
    const port = await navigator.serial.requestPort({ filters: BOOTLOADER_HINTS });
    log(`シリアルポートに接続 [${half}]`);
    const { firmware, initPacket } = await parseDfuPackage(blob);
    setStatus(statusEl, '書き込み中…');
    setProgress(bar, 0);
    await flash(port, {
      firmware,
      initPacket,
      onProgress: (r) => setProgress(bar, r),
      onLog: (m) => log(`[${half}] ${m}`),
    });
    setStatus(
      statusEl,
      `書き込み完了${firmwareLabel ? ` (${firmwareLabel})` : ''}`,
      'ok',
    );
  } catch (e) {
    setStatus(statusEl, '失敗', 'err');
    log(`エラー [${half}]: ${e instanceof Error ? e.message : e}`);
  }
}

// ---------------------------------------------------------------------------
// Wire up
// ---------------------------------------------------------------------------

checkCompat();

ui.btnFetch.addEventListener('click', () => loadBundledFirmware());
ui.fileCentral.addEventListener('change', (e) => {
  const file = /** @type {HTMLInputElement} */ (e.target).files?.[0];
  if (file) loadFirmwareFromFile('central', file);
});
ui.filePeripheral.addEventListener('change', (e) => {
  const file = /** @type {HTMLInputElement} */ (e.target).files?.[0];
  if (file) loadFirmwareFromFile('peripheral', file);
});
ui.btnResetCentral.addEventListener('click', resetCentral);
ui.btnFlashCentral.addEventListener('click', () => flashHalf('central'));
ui.btnFlashPeripheral.addEventListener('click', () => flashHalf('peripheral'));

// After compat check we can enable the HID/Serial buttons.
if ('hid' in navigator) ui.btnResetCentral.disabled = false;
if ('serial' in navigator) {
  ui.btnFlashCentral.disabled = false;
  ui.btnFlashPeripheral.disabled = false;
}
