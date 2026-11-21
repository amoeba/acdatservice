/**
 * ACSP v1 per-setup pack format: the encoder and decoder.
 *
 * A pack holds one Setup's portal files as raw DAT bytes, so the client can load
 * the Setup from one HTTP object. Layout, contents, and the read contract are in
 * `docs/setup-packs.md`.
 *
 * Browser-safe: no Node dependencies, so the browser can decode packs too.
 */

export const ACSP_HEADER_LEN = 16;
export const ACSP_ENTRY_LEN = 12;
export const ACSP_VERSION = 1;
export const ACSP_KIND_SETUP = 3;
/** `length` sentinel for an id known to be absent from the DAT. */
export const ACSP_MISSING = 0xffffffff;

export interface AcspEntry {
  id: number;
  /** Raw DAT file bytes, or `undefined` for a known-missing id. */
  data: Uint8Array | undefined;
}

export interface DecodedSetupPack {
  setupId: number;
  /** Present ids map to a view; known-missing ids map to `undefined`. */
  entries: Map<number, Uint8Array | undefined>;
}

/** Encode a setup pack. `entries` should be sorted ascending by id. */
export function encodeSetupPack(setupId: number, entries: AcspEntry[]): Uint8Array {
  const payloadBytes = entries.reduce((n, e) => n + (e.data?.byteLength ?? 0), 0);
  const out = new Uint8Array(ACSP_HEADER_LEN + entries.length * ACSP_ENTRY_LEN + payloadBytes);
  const dv = new DataView(out.buffer);

  out[0] = 0x41; // 'A'
  out[1] = 0x43; // 'C'
  out[2] = 0x53; // 'S'
  out[3] = 0x50; // 'P'
  out[4] = ACSP_VERSION;
  out[5] = ACSP_KIND_SETUP;
  out[6] = 0; // reserved
  out[7] = 0;
  dv.setUint32(8, setupId >>> 0, true);
  dv.setUint32(12, entries.length, true);

  let payloadAt = ACSP_HEADER_LEN + entries.length * ACSP_ENTRY_LEN;
  entries.forEach((entry, index) => {
    const at = ACSP_HEADER_LEN + index * ACSP_ENTRY_LEN;
    dv.setUint32(at, entry.id >>> 0, true);
    if (entry.data === undefined) {
      dv.setUint32(at + 4, 0, true);
      dv.setUint32(at + 8, ACSP_MISSING, true);
    } else {
      dv.setUint32(at + 4, payloadAt, true);
      dv.setUint32(at + 8, entry.data.byteLength, true);
      out.set(entry.data, payloadAt);
      payloadAt += entry.data.byteLength;
    }
  });

  return out;
}

/** Decode a setup pack, validating every invariant in the format spec. */
export function decodeSetupPack(bytes: Uint8Array): DecodedSetupPack {
  if (bytes.byteLength < ACSP_HEADER_LEN) {
    throw new Error(`ACSP: too short (${bytes.byteLength} bytes)`);
  }
  if (!(bytes[0] === 0x41 && bytes[1] === 0x43 && bytes[2] === 0x53 && bytes[3] === 0x50)) {
    throw new Error("ACSP: bad magic");
  }
  if (bytes[4] !== ACSP_VERSION) {
    throw new Error(`ACSP: unsupported version ${bytes[4]}`);
  }
  if (bytes[5] !== ACSP_KIND_SETUP) {
    throw new Error(`ACSP: invalid kind ${bytes[5]}`);
  }

  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (dv.getUint16(6, true) !== 0) {
    throw new Error("ACSP: reserved must be zero");
  }
  const setupId = dv.getUint32(8, true);
  const count = dv.getUint32(12, true);
  const tableEnd = ACSP_HEADER_LEN + count * ACSP_ENTRY_LEN;
  if (bytes.byteLength < tableEnd) {
    throw new Error("ACSP: truncated entry table");
  }

  const entries = new Map<number, Uint8Array | undefined>();
  const ranges: Array<[number, number]> = [];
  for (let i = 0; i < count; i++) {
    const at = ACSP_HEADER_LEN + i * ACSP_ENTRY_LEN;
    const id = dv.getUint32(at, true);
    const offset = dv.getUint32(at + 4, true);
    const length = dv.getUint32(at + 8, true);
    if (entries.has(id)) {
      throw new Error(`ACSP: duplicate id 0x${id.toString(16)}`);
    }
    if (length === ACSP_MISSING) {
      // Known-missing: no payload, offset is ignored.
      entries.set(id, undefined);
      continue;
    }
    if (offset < tableEnd || offset + length > bytes.byteLength) {
      throw new Error(`ACSP: entry 0x${id.toString(16)} out of bounds`);
    }
    entries.set(id, bytes.subarray(offset, offset + length));
    ranges.push([offset, offset + length]);
  }
  // Present payloads must tile [tableEnd, fileSize) with no gap, overlap, or
  // trailing bytes. Order-independent.
  ranges.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  let cursor = tableEnd;
  for (const [start, end] of ranges) {
    if (start !== cursor) {
      throw new Error("ACSP: payload ranges do not tile exactly");
    }
    cursor = end;
  }
  if (cursor !== bytes.byteLength) {
    throw new Error("ACSP: trailing bytes after payloads");
  }

  return { setupId, entries };
}

/** True only when the magic, version, and kind all match ACSP v1. */
export function isSetupPack(bytes: Uint8Array): boolean {
  return (
    bytes.byteLength >= ACSP_HEADER_LEN &&
    bytes[0] === 0x41 &&
    bytes[1] === 0x43 &&
    bytes[2] === 0x53 &&
    bytes[3] === 0x50 &&
    bytes[4] === ACSP_VERSION &&
    bytes[5] === ACSP_KIND_SETUP
  );
}
