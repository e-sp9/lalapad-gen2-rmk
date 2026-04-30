// WebHID → Vial/VIA "jump to bootloader" (command 0x0B).
//
// RMK handles `ViaCommand::BootloaderJump (0x0B)` by writing
// `NRF_POWER->GPREGRET = 0x57` and issuing a soft reset, which makes the
// Adafruit UF2 bootloader stay in DFU mode after the jump.
//
// The Vial raw HID interface exposes usage page 0xFF60 / usage 0x61.
//
// LaLaPad Gen2 RMK is a split BLE keyboard: only the central (right half)
// runs RMK's USB stack, so jump-to-bootloader works only for the central.
// The peripheral (left half) must be put into DFU mode by double-tapping
// reset on the XIAO BLE.

const VIAL_USAGE_PAGE = 0xff60;
const VIAL_USAGE = 0x61;

// VIA command IDs.
const ID_BOOTLOADER_JUMP = 0x0b;
const ID_VIAL_PREFIX = 0xfe;

// Vial sub-command IDs (see RMK rmk-types/src/protocol/vial.rs).
const VIAL_GET_UNLOCK_STATUS = 0x05;

// Raw HID report size expected by RMK's Vial interface.
const REPORT_SIZE = 32;

function buildReport(bytes) {
  const report = new Uint8Array(REPORT_SIZE);
  report.set(bytes);
  return report;
}

async function sendCommand(device, bytes) {
  const report = buildReport(bytes);
  await device.sendReport(0, report);
  return waitForReport(device);
}

function waitForReport(device, timeoutMs = 1500) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      device.removeEventListener('inputreport', handler);
      reject(new Error('HID report timeout'));
    }, timeoutMs);
    const handler = (e) => {
      clearTimeout(timer);
      device.removeEventListener('inputreport', handler);
      resolve(new Uint8Array(e.data.buffer));
    };
    device.addEventListener('inputreport', handler);
  });
}

export async function requestVialDevice(filters) {
  if (!('hid' in navigator)) {
    throw new Error('WebHID is not supported in this browser.');
  }
  const hintFilters = (filters || []).map((f) => ({
    ...f,
    usagePage: VIAL_USAGE_PAGE,
    usage: VIAL_USAGE,
  }));
  const devices = await navigator.hid.requestDevice({
    filters: hintFilters.length
      ? hintFilters
      : [{ usagePage: VIAL_USAGE_PAGE, usage: VIAL_USAGE }],
  });
  if (!devices.length) return null;
  const device = devices[0];
  if (!device.opened) await device.open();
  return device;
}

// Read Vial's unlock status. Response byte 0 is 1 if unlocked.
export async function isUnlocked(device) {
  try {
    const resp = await sendCommand(device, [ID_VIAL_PREFIX, VIAL_GET_UNLOCK_STATUS]);
    return resp[0] === 1;
  } catch {
    return true;
  }
}

// Fire the VIA "bootloader jump" command. The device resets immediately and
// the HID interface disappears; we do not wait for a response.
export async function jumpToBootloader(device) {
  const report = buildReport([ID_BOOTLOADER_JUMP]);
  await device.sendReport(0, report);
}
