import assert from "node:assert/strict";
import test from "node:test";
import { ACLB_HEADER_LEN, BundleKind, decodeBundle, encodeBundle } from "../src/aclb.js";

const bytesFromHex = (hex: string): Uint8Array =>
  Uint8Array.from(hex.split(/\s+/).filter(Boolean).map((b) => Number.parseInt(b, 16)));

const clone = (bytes: Uint8Array): Uint8Array => new Uint8Array(bytes);

test("round-trips both bundle kinds", () => {
  for (const kind of [BundleKind.Landblock, BundleKind.Envcell]) {
    const bytes = encodeBundle(kind, 0x00ae, [
      { id: 0x00ae0100, data: new Uint8Array([0xde, 0xad]) },
      { id: 0x00ae0101, data: new Uint8Array([0xbe, 0xef, 0x00]) },
    ]);
    const decoded = decodeBundle(bytes);
    assert.equal(decoded.kind, kind);
    assert.equal(decoded.landblock, 0x00ae);
    assert.deepEqual(decoded.entries.get(0x00ae0100), new Uint8Array([0xde, 0xad]));
    assert.deepEqual(decoded.entries.get(0x00ae0101), new Uint8Array([0xbe, 0xef, 0x00]));
  }
});

test("count=0 is a valid 16-byte header", () => {
  const bytes = encodeBundle(BundleKind.Landblock, 0x00ae, []);
  assert.equal(bytes.byteLength, ACLB_HEADER_LEN);
  const decoded = decodeBundle(bytes);
  assert.equal(decoded.entries.size, 0);
});

test("count=0 rejects trailing bytes after the header", () => {
  const bytes = encodeBundle(BundleKind.Landblock, 0x00ae, []);
  const withTrailing = new Uint8Array(bytes.byteLength + 1);
  withTrailing.set(bytes);
  assert.throws(() => decodeBundle(withTrailing), /trailing bytes/);
});

test("zero-length entries tile correctly", () => {
  const bytes = encodeBundle(BundleKind.Envcell, 0x00ae, [
    { id: 0x00ae0100, data: new Uint8Array(0) },
    { id: 0x00ae0101, data: new Uint8Array([1, 2]) },
  ]);
  const decoded = decodeBundle(bytes);
  assert.deepEqual(decoded.entries.get(0x00ae0100), new Uint8Array(0));
  assert.deepEqual(decoded.entries.get(0x00ae0101), new Uint8Array([1, 2]));
});

test("decodes the spec worked example", () => {
  const bytes = bytesFromHex(`
    41 43 4C 42 01 02 00 00 AE 00 00 00 02 00 00 00
    00 01 AE 00 28 00 00 00 02 00 00 00 01 01 AE 00
    2A 00 00 00 03 00 00 00 DE AD BE EF 00
  `);
  const decoded = decodeBundle(bytes);
  assert.equal(decoded.kind, BundleKind.Envcell);
  assert.equal(decoded.landblock, 0x00ae);
  assert.deepEqual(decoded.entries.get(0x00ae0100), new Uint8Array([0xde, 0xad]));
  assert.deepEqual(decoded.entries.get(0x00ae0101), new Uint8Array([0xbe, 0xef, 0x00]));
});

test("rejects invalid bundles", () => {
  const base = encodeBundle(BundleKind.Envcell, 0x00ae, [
    { id: 0x00ae0100, data: new Uint8Array([1, 2]) },
    { id: 0x00ae0101, data: new Uint8Array([3, 4, 5]) },
  ]);

  // 1. fewer than 16 bytes
  assert.throws(() => decodeBundle(new Uint8Array(10)), /too short/);

  // 2. bad magic
  let b = clone(base);
  b[0] = 0;
  assert.throws(() => decodeBundle(b), /bad magic/);

  // 3. version != 1
  b = clone(base);
  b[4] = 2;
  assert.throws(() => decodeBundle(b), /unsupported version/);

  // 4. kind not 1 or 2
  b = clone(base);
  b[5] = 0;
  assert.throws(() => decodeBundle(b), /invalid kind/);
  b = clone(base);
  b[5] = 3;
  assert.throws(() => decodeBundle(b), /invalid kind/);

  // 5. reserved != 0
  b = clone(base);
  b[6] = 1;
  assert.throws(() => decodeBundle(b), /reserved/);

  // 6. landblock > 0xFFFF
  b = clone(base);
  new DataView(b.buffer).setUint32(8, 0x1_0000, true);
  assert.throws(() => decodeBundle(b), /landblock out of range/);

  // 7. truncated entry table
  assert.throws(() => decodeBundle(base.slice(0, 20)), /truncated entry table/);

  // 8. payload past end
  b = clone(base);
  new DataView(b.buffer).setUint32(ACLB_HEADER_LEN + 8, 100, true);
  assert.throws(() => decodeBundle(b), /out of bounds/);

  // 9. id>>16 != landblock
  b = clone(base);
  new DataView(b.buffer).setUint32(ACLB_HEADER_LEN, 0x00af0100, true);
  assert.throws(() => decodeBundle(b), /landblock mismatch/);

  // 10. offset points into the table
  b = clone(base);
  new DataView(b.buffer).setUint32(ACLB_HEADER_LEN + 4, 20, true);
  assert.throws(() => decodeBundle(b), /out of bounds/);

  // 11a. gap between payloads
  b = clone(base);
  new DataView(b.buffer).setUint32(ACLB_HEADER_LEN + 12 + 4, 43, true);
  new DataView(b.buffer).setUint32(ACLB_HEADER_LEN + 12 + 8, 2, true);
  assert.throws(() => decodeBundle(b), /do not tile exactly/);

  // 11b. overlap
  b = clone(base);
  new DataView(b.buffer).setUint32(ACLB_HEADER_LEN + 12 + 4, 41, true);
  assert.throws(() => decodeBundle(b), /do not tile exactly/);

  // 11c. trailing bytes
  b = new Uint8Array(base.byteLength + 1);
  b.set(base);
  assert.throws(() => decodeBundle(b), /trailing bytes/);
});
