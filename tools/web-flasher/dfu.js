// Nordic Legacy DFU over WebSerial, compatible with the Adafruit nRF52 UF2
// bootloader (used by Seeed XIAO nRF52840 / XIAO BLE). Protocol is ported
// from adafruit-nrfutil's
// `nordicsemi/dfu/dfu_transport_serial.py` (DfuTransportSerial / HciPacket).

import {
  calcCrc16,
  concat,
  slipEncodeEscChars,
  slipHeader,
  u16le,
  u32le,
} from './slip.js';

// HCI control bits for Reliable Packet Type 14 frames.
const DATA_INTEGRITY_CHECK_PRESENT = 1;
const RELIABLE_PACKET = 1;
const HCI_PACKET_TYPE = 14;

// DFU payload kinds (first u32 of the frame content).
const DFU_INIT_PACKET = 1;
const DFU_START_PACKET = 3;
const DFU_DATA_PACKET = 4;
const DFU_STOP_DATA_PACKET = 5;

// Update modes.
export const DFU_UPDATE_MODE_APP = 4;

const DFU_PACKET_MAX_SIZE = 512;
const FLASH_PAGE_SIZE = 4096;
const FLASH_PAGE_ERASE_TIME_MS = 89.7; // nrf52840 worst case
const FLASH_WORD_WRITE_TIME_MS = 0.1;
const FLASH_PAGE_WRITE_TIME_MS = (FLASH_PAGE_SIZE / 4) * FLASH_WORD_WRITE_TIME_MS;

export const DEFAULT_BAUD_RATE = 115200;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// HciPacket state: module-level sequence number, rolls over mod 8, matches
// the bootloader's expected counter.
let sequenceNumber = 0;

export function resetSequenceNumber() {
  sequenceNumber = 0;
}

// Build an HCI "reliable" frame carrying `payload`:
//   [0xC0] [SLIP header(4) | payload | CRC16 LE, with 0xC0/0xDB escaped] [0xC0]
function buildHciPacket(payload) {
  sequenceNumber = (sequenceNumber + 1) % 8;
  const header = slipHeader(
    sequenceNumber,
    DATA_INTEGRITY_CHECK_PRESENT,
    RELIABLE_PACKET,
    HCI_PACKET_TYPE,
    payload.length,
  );
  const body = concat(header, payload);
  const crc = calcCrc16(body, 0xffff);
  const withCrc = concat(body, u16le(crc));
  const escaped = slipEncodeEscChars(withCrc);
  return concat(new Uint8Array([0xc0]), escaped, new Uint8Array([0xc0]));
}

// Incremental SLIP frame reader: feeds raw bytes in, yields complete frames
// delimited by a pair of 0xC0 bytes.
class FrameAccumulator {
  constructor() {
    this.buf = [];
    this.inFrame = false;
  }
  push(bytes) {
    const frames = [];
    for (let i = 0; i < bytes.length; i++) {
      const b = bytes[i];
      if (b === 0xc0) {
        if (this.inFrame) {
          if (this.buf.length > 0) {
            frames.push(new Uint8Array(this.buf));
          }
          this.buf = [];
          this.inFrame = false;
        } else {
          this.inFrame = true;
          this.buf = [];
        }
      } else if (this.inFrame) {
        this.buf.push(b);
      }
    }
    return frames;
  }
}

// Wraps a WritableStream + ReadableStream pair. Reader is kept open for the
// lifetime of the transport so we can consume ACKs in order.
class SerialIO {
  constructor(port) {
    this.port = port;
    this.writer = port.writable.getWriter();
    this.reader = port.readable.getReader();
    this.accumulator = new FrameAccumulator();
    this.pendingFrames = [];
    this.pendingResolvers = [];
    this.totalRx = 0;
    this.readLoop();
  }
  async readLoop() {
    try {
      while (true) {
        const { value, done } = await this.reader.read();
        if (done) break;
        if (!value || value.length === 0) continue;
        this.totalRx += value.length;
        console.debug('[dfu] RX', value.length, 'B:', hexPreview(value, 32));
        const frames = this.accumulator.push(value);
        for (const f of frames) {
          const resolver = this.pendingResolvers.shift();
          if (resolver) resolver.resolve(f);
          else this.pendingFrames.push(f);
        }
      }
    } catch (err) {
      // Port disconnected mid-transfer — reject any waiters.
      const e = err instanceof Error ? err : new Error(String(err));
      for (const r of this.pendingResolvers) r.reject?.(e);
      this.pendingResolvers = [];
    }
  }
  write(bytes) {
    return this.writer.write(bytes);
  }
  nextFrame(timeoutMs) {
    if (this.pendingFrames.length) {
      return Promise.resolve(this.pendingFrames.shift());
    }
    return new Promise((resolve, reject) => {
      const slot = { resolve, reject };
      this.pendingResolvers.push(slot);
      if (timeoutMs != null) {
        setTimeout(() => {
          const idx = this.pendingResolvers.indexOf(slot);
          if (idx >= 0) {
            this.pendingResolvers.splice(idx, 1);
            reject(new Error(`ACK timeout after ${timeoutMs}ms`));
          }
        }, timeoutMs);
      }
    });
  }
  // Swallow any RX bytes arriving within `ms`, used to clear bootloader banners
  // / boot-time junk before we start the handshake. After waiting we also
  // reset the frame accumulator so stray 0xC0 bytes don't leave it in the
  // middle of an imaginary frame.
  async drain(ms) {
    const start = Date.now();
    while (Date.now() - start < ms) {
      await sleep(20);
    }
    const count = this.pendingFrames.reduce((s, f) => s + f.length + 2, 0);
    this.pendingFrames = [];
    this.accumulator = new FrameAccumulator();
    return count;
  }
  async close() {
    try {
      await this.writer.close();
    } catch {}
    try {
      this.writer.releaseLock();
    } catch {}
    try {
      await this.reader.cancel();
    } catch {}
    try {
      this.reader.releaseLock();
    } catch {}
  }
}

async function writeWithTimeout(io, bytes, timeoutMs) {
  const writePromise = io.write(bytes);
  const timeoutPromise = new Promise((_, reject) =>
    setTimeout(() => reject(new Error(`write() stalled after ${timeoutMs}ms — port may not be the bootloader, or DTR/flow control is blocking`)), timeoutMs),
  );
  await Promise.race([writePromise, timeoutPromise]);
}

async function sendPacket(io, packet, ackTimeoutMs = 2000) {
  console.debug('[dfu] TX', packet.length, 'B:', hexPreview(packet, 32));
  await writeWithTimeout(io, packet, 3000);
  // Drain one ACK frame. We don't verify the ACK sequence number; adafruit-nrfutil
  // doesn't either in practice (the retry path is effectively disabled upstream).
  const frame = await io.nextFrame(ackTimeoutMs);
  console.debug('[dfu] ACK', frame.length, 'B:', hexPreview(frame, 16));
}

function hexPreview(bytes, max) {
  const n = Math.min(bytes.length, max);
  const parts = [];
  for (let i = 0; i < n; i++) parts.push(bytes[i].toString(16).padStart(2, '0'));
  return parts.join(' ') + (bytes.length > n ? ` … (+${bytes.length - n}B)` : '');
}

function startPacketFrame(mode, sdSize, blSize, appSize) {
  return concat(
    u32le(DFU_START_PACKET),
    u32le(mode),
    u32le(sdSize),
    u32le(blSize),
    u32le(appSize),
  );
}
function initPacketFrame(initPacket) {
  // Trailing 2-byte padding required by the bootloader parser.
  return concat(u32le(DFU_INIT_PACKET), initPacket, u16le(0x0000));
}
function dataPacketFrame(chunk) {
  return concat(u32le(DFU_DATA_PACKET), chunk);
}
function stopDataPacketFrame() {
  return u32le(DFU_STOP_DATA_PACKET);
}

function erasePagesFor(appSize) {
  return Math.max(1, Math.floor(appSize / FLASH_PAGE_SIZE) + 1);
}

// Perform the full DFU session. `firmware` and `initPacket` are Uint8Arrays,
// extracted from the DFU zip (`*.bin` / `*.dat`).
//
// Progress callback receives values 0..1 during the firmware transfer.
export async function flash(port, { firmware, initPacket, onProgress, onLog }) {
  const log = (msg) => onLog?.(msg);
  resetSequenceNumber();
  const info = port.getInfo?.() ?? {};
  if (info.usbVendorId || info.usbProductId) {
    log(`選択ポート: USB VID=0x${(info.usbVendorId ?? 0).toString(16)} PID=0x${(info.usbProductId ?? 0).toString(16)}`);
  }
  log(`ポートを ${DEFAULT_BAUD_RATE} bps で open`);
  await port.open({ baudRate: DEFAULT_BAUD_RATE });
  log('ポート open 完了');
  // Match adafruit-nrfutil's open(): toggle DTR to reset the bootloader's DFU
  // state machine before we start talking, then let it boot back up. Without
  // this the bootloader often ignores the first START_DFU.
  try {
    await port.setSignals({ dataTerminalReady: false });
    await sleep(50);
    await port.setSignals({ dataTerminalReady: true });
    log('DTR トグル完了');
  } catch (e) {
    log(`DTR 制御に失敗 (継続): ${e instanceof Error ? e.message : e}`);
  }
  await sleep(200);
  const io = new SerialIO(port);
  // Drain any banner/junk the bootloader may have emitted between reset and
  // our first write so it does not get parsed as an ACK for START_DFU.
  const drained = await io.drain(150);
  if (drained) log(`初期受信 ${drained} bytes を破棄`);

  try {
    log('送信: START_DFU');
    // START_DFU triggers a flash erase whose ACK can lag; give it extra slack.
    await sendPacket(
      io,
      buildHciPacket(startPacketFrame(DFU_UPDATE_MODE_APP, 0, 0, firmware.length)),
      8000,
    );
    const erasePages = erasePagesFor(firmware.length);
    const eraseWaitMs = Math.max(500, erasePages * FLASH_PAGE_ERASE_TIME_MS);
    log(`フラッシュ消去待機: ${erasePages} pages (${Math.round(eraseWaitMs)}ms)`);
    await sleep(eraseWaitMs);

    log(`送信: INIT_PACKET (${initPacket.length} bytes)`);
    await sendPacket(io, buildHciPacket(initPacketFrame(initPacket)));

    log(`送信: DATA ${firmware.length} bytes, ${DFU_PACKET_MAX_SIZE}B/chunk`);
    let sent = 0;
    let chunkIndex = 0;
    for (let off = 0; off < firmware.length; off += DFU_PACKET_MAX_SIZE) {
      const end = Math.min(off + DFU_PACKET_MAX_SIZE, firmware.length);
      const chunk = firmware.subarray(off, end);
      await sendPacket(io, buildHciPacket(dataPacketFrame(chunk)));
      sent = end;
      chunkIndex++;
      onProgress?.(sent / firmware.length);
      // Every 8 chunks (one 4KB flash page), the bootloader erases/writes —
      // its CPU stalls briefly, so insert a matching cooldown.
      if (chunkIndex % 8 === 0) {
        await sleep(FLASH_PAGE_WRITE_TIME_MS);
      }
    }
    await sleep(FLASH_PAGE_WRITE_TIME_MS);

    log('送信: STOP_DATA');
    try {
      await sendPacket(io, buildHciPacket(stopDataPacketFrame()), 3000);
    } catch (e) {
      // Bootloader may reset into the app before ACKing — tolerate.
      log(`STOP_DATA ACK 無し (デバイスが再起動中の可能性): ${e.message}`);
    }
    onProgress?.(1);
    log('書き込み完了 — デバイスは自動的に再起動します');
  } finally {
    await io.close();
    try {
      await port.close();
    } catch {}
  }
}

// Parse a DFU zip blob (downloaded from GitHub Releases or chosen locally).
// Returns { firmware: Uint8Array, initPacket: Uint8Array, manifest: object }.
export async function parseDfuPackage(zipBlob) {
  const JSZip = /** @type {any} */ (globalThis).JSZip;
  if (!JSZip) {
    throw new Error('JSZip is not loaded.');
  }
  const zip = await JSZip.loadAsync(zipBlob);
  const manifestEntry = zip.file('manifest.json');
  if (!manifestEntry) throw new Error('manifest.json not found in DFU zip');
  const manifest = JSON.parse(await manifestEntry.async('string'));
  const app = manifest.manifest?.application;
  if (!app) throw new Error('manifest.application missing');
  const binEntry = zip.file(app.bin_file);
  const datEntry = zip.file(app.dat_file);
  if (!binEntry || !datEntry) {
    throw new Error(`bin/dat not found: ${app.bin_file} / ${app.dat_file}`);
  }
  const firmware = new Uint8Array(await binEntry.async('arraybuffer'));
  const initPacket = new Uint8Array(await datEntry.async('arraybuffer'));
  return { firmware, initPacket, manifest };
}
