// SLIP framing + HCI header builder + CRC16, derived from
// adafruit/Adafruit_nRF52_nrfutil (nordicsemi/dfu/util.py, crc16.py).

// Encode SLIP escape chars: 0xC0 -> DB DC, 0xDB -> DB DD.
export function slipEncodeEscChars(data) {
  const out = [];
  for (const b of data) {
    if (b === 0xc0) out.push(0xdb, 0xdc);
    else if (b === 0xdb) out.push(0xdb, 0xdd);
    else out.push(b);
  }
  return new Uint8Array(out);
}

// Decode SLIP escape chars: DB DC -> 0xC0, DB DD -> 0xDB.
export function slipDecodeEscChars(data) {
  const out = [];
  let i = 0;
  while (i < data.length) {
    const c = data[i++];
    if (c === 0xdb) {
      const c2 = data[i++];
      if (c2 === 0xdc) out.push(0xc0);
      else if (c2 === 0xdd) out.push(0xdb);
      else throw new Error(`SLIP escape byte 0xDB not followed by 0xDC/0xDD (got 0x${c2?.toString(16)})`);
    } else {
      out.push(c);
    }
  }
  return new Uint8Array(out);
}

// Build the 4-byte HCI SLIP preamble.
//   byte0: seq (3b) | ((seq+1)%8)<<3 | dip<<6 | rp<<7
//   byte1: pkt_type (4b) | (pkt_len & 0x00F)<<4
//   byte2: (pkt_len & 0xFF0)>>4
//   byte3: two's-complement checksum over bytes 0..2
export function slipHeader(seq, dip, rp, pktType, pktLen) {
  const a = new Uint8Array(4);
  a[0] = (seq | (((seq + 1) % 8) << 3) | (dip << 6) | (rp << 7)) & 0xff;
  a[1] = (pktType | ((pktLen & 0x000f) << 4)) & 0xff;
  a[2] = ((pktLen & 0x0ff0) >> 4) & 0xff;
  a[3] = (-(a[0] + a[1] + a[2])) & 0xff;
  return a;
}

// CRC16-CCITT (0xFFFF seed) as used by Nordic Legacy DFU.
//
// The Python reference in nordicsemi/dfu/crc16.py lets `crc` accumulate bits
// above 16 inside a single iteration; those bits are always discarded at the
// start of the next iteration's byte swap, so a 32-bit intermediate (what JS
// bitwise ops give us natively) is equivalent.
export function calcCrc16(data, crc = 0xffff) {
  for (let i = 0; i < data.length; i++) {
    const b = data[i];
    crc = ((crc >>> 8) & 0x00ff) | ((crc << 8) & 0xff00);
    crc ^= b;
    crc ^= (crc & 0x00ff) >>> 4;
    crc ^= crc << 12;
    crc ^= ((crc & 0x00ff) << 4) << 1;
  }
  return crc & 0xffff;
}

// Concatenate Uint8Arrays into one.
export function concat(...parts) {
  let len = 0;
  for (const p of parts) len += p.length;
  const out = new Uint8Array(len);
  let off = 0;
  for (const p of parts) {
    out.set(p, off);
    off += p.length;
  }
  return out;
}

// Little-endian helpers.
export function u32le(v) {
  const a = new Uint8Array(4);
  a[0] = v & 0xff;
  a[1] = (v >> 8) & 0xff;
  a[2] = (v >> 16) & 0xff;
  a[3] = (v >>> 24) & 0xff;
  return a;
}
export function u16le(v) {
  return new Uint8Array([v & 0xff, (v >> 8) & 0xff]);
}
