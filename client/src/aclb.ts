/**
 * ACLB v1 per-landblock bundle format — TypeScript reference.
 *
 * Byte-for-byte mirror of `acdatservice/src/bundle.rs`. The authoritative
 * specification is `acdatservice/docs/bundle-format.md`. The bundle benchmark
 * harness uses this to pack real origin files exactly the way the Rust
 * generator does. This copy lives under `src/` so the browser can decode
 * bundles too; it has no Node dependencies.
 */

export enum BundleKind {
  Landblock = 1,
  Envcell = 2,
}

export const ACLB_HEADER_LEN = 16;
export const ACLB_ENTRY_LEN = 12;

export interface BundleEntry {
  id: number;
  data: Uint8Array;
}

/** Encode a bundle. `entries` should be sorted ascending by id. */
export function encodeBundle(
  kind: BundleKind,
  landblock: number,
  entries: BundleEntry[],
): Uint8Array {
  const payloadBytes = entries.reduce((n, e) => n + e.data.byteLength, 0);
  const out = new Uint8Array(ACLB_HEADER_LEN + entries.length * ACLB_ENTRY_LEN + payloadBytes);
  const dv = new DataView(out.buffer);

  out[0] = 0x41; // 'A'
  out[1] = 0x43; // 'C'
  out[2] = 0x4c; // 'L'
  out[3] = 0x42; // 'B'
  out[4] = 1; // version
  out[5] = kind;
  out[6] = 0; // reserved
  out[7] = 0;
  dv.setUint32(8, landblock >>> 0, true);
  dv.setUint32(12, entries.length, true);

  let payloadAt = ACLB_HEADER_LEN + entries.length * ACLB_ENTRY_LEN;
  entries.forEach((entry, index) => {
    const at = ACLB_HEADER_LEN + index * ACLB_ENTRY_LEN;
    dv.setUint32(at, entry.id >>> 0, true);
    dv.setUint32(at + 4, payloadAt, true);
    dv.setUint32(at + 8, entry.data.byteLength, true);
    out.set(entry.data, payloadAt);
    payloadAt += entry.data.byteLength;
  });

  return out;
}

export interface DecodedBundle {
  kind: BundleKind;
  landblock: number;
  entries: Map<number, Uint8Array>;
}

/** Decode a bundle, validating every invariant in the format spec. */
export function decodeBundle(bytes: Uint8Array): DecodedBundle {
  if (bytes.byteLength < ACLB_HEADER_LEN) {
    throw new Error(`ACLB: too short (${bytes.byteLength} bytes)`);
  }
  if (!(bytes[0] === 0x41 && bytes[1] === 0x43 && bytes[2] === 0x4c && bytes[3] === 0x42)) {
    throw new Error("ACLB: bad magic");
  }
  if (bytes[4] !== 1) {
    throw new Error(`ACLB: unsupported version ${bytes[4]}`);
  }

  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const kind = bytes[5];
  if (kind !== BundleKind.Landblock && kind !== BundleKind.Envcell) {
    throw new Error(`ACLB: invalid kind ${kind}`);
  }
  if (dv.getUint16(6, true) !== 0) {
    throw new Error("ACLB: reserved must be zero");
  }
  const landblock = dv.getUint32(8, true);
  if (landblock > 0xffff) {
    throw new Error("ACLB: landblock out of range");
  }
  const count = dv.getUint32(12, true);
  const tableEnd = ACLB_HEADER_LEN + count * ACLB_ENTRY_LEN;
  if (bytes.byteLength < tableEnd) {
    throw new Error("ACLB: truncated entry table");
  }

  const entries = new Map<number, Uint8Array>();
  const ranges: Array<[number, number]> = [];
  for (let i = 0; i < count; i++) {
    const at = ACLB_HEADER_LEN + i * ACLB_ENTRY_LEN;
    const id = dv.getUint32(at, true);
    const offset = dv.getUint32(at + 4, true);
    const length = dv.getUint32(at + 8, true);
    if (id >>> 16 !== landblock) {
      throw new Error(`ACLB: entry 0x${id.toString(16)} landblock mismatch`);
    }
    if (offset < tableEnd || offset + length > bytes.byteLength) {
      throw new Error(`ACLB: entry 0x${id.toString(16)} out of bounds`);
    }
    entries.set(id, bytes.subarray(offset, offset + length));
    ranges.push([offset, offset + length]);
  }
  // Invariant 11: payloads tile [tableEnd, fileSize) with no gap, overlap, or
  // trailing bytes. Order-independent.
  ranges.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  let cursor = tableEnd;
  for (const [start, end] of ranges) {
    if (start !== cursor) {
      throw new Error("ACLB: payload ranges do not tile exactly");
    }
    cursor = end;
  }
  if (cursor !== bytes.byteLength) {
    throw new Error("ACLB: trailing bytes after payloads");
  }

  return { kind, landblock, entries };
}
