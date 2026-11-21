import assert from "node:assert/strict";
import test from "node:test";
import {
  ACSP_HEADER_LEN,
  decodeSetupPack,
  encodeSetupPack,
  isSetupPack,
} from "../src/acsp.js";

const bytesFromHex = (hex: string): Uint8Array =>
  Uint8Array.from(hex.split(/\s+/).filter(Boolean).map((b) => Number.parseInt(b, 16)));

const clone = (bytes: Uint8Array): Uint8Array => new Uint8Array(bytes);

test("round-trips present, missing, and absent ids", () => {
  const bytes = encodeSetupPack(0x02001b7f, [
    { id: 0x02000001, data: new Uint8Array([0xde, 0xad]) },
    { id: 0x02000002, data: undefined },
    { id: 0x02000003, data: new Uint8Array([0xbe, 0xef, 0x00]) },
  ]);
  const decoded = decodeSetupPack(bytes);
  assert.equal(decoded.setupId, 0x02001b7f);
  assert.deepEqual(decoded.entries.get(0x02000001), new Uint8Array([0xde, 0xad]));
  assert.ok(decoded.entries.has(0x02000002));
  assert.equal(decoded.entries.get(0x02000002), undefined);
  assert.deepEqual(decoded.entries.get(0x02000003), new Uint8Array([0xbe, 0xef, 0x00]));
  assert.ok(!decoded.entries.has(0x02000004));
});

test("count=0 is a valid 16-byte header", () => {
  const bytes = encodeSetupPack(0x02001b7f, []);
  assert.equal(bytes.byteLength, ACSP_HEADER_LEN);
  const decoded = decodeSetupPack(bytes);
  assert.equal(decoded.setupId, 0x02001b7f);
  assert.equal(decoded.entries.size, 0);
});

test("count=0 rejects trailing bytes after the header", () => {
  const bytes = encodeSetupPack(0x02001b7f, []);
  const withTrailing = new Uint8Array(bytes.byteLength + 1);
  withTrailing.set(bytes);
  assert.throws(() => decodeSetupPack(withTrailing), /trailing bytes/);
});

test("zero-length present entries tile correctly", () => {
  const bytes = encodeSetupPack(0x02001b7f, [
    { id: 0x02000001, data: new Uint8Array(0) },
    { id: 0x02000002, data: new Uint8Array([1, 2]) },
  ]);
  const decoded = decodeSetupPack(bytes);
  assert.deepEqual(decoded.entries.get(0x02000001), new Uint8Array(0));
  assert.deepEqual(decoded.entries.get(0x02000002), new Uint8Array([1, 2]));
});

test("payloads are views, not copies", () => {
  const bytes = encodeSetupPack(0x02001b7f, [
    { id: 0x02000001, data: new Uint8Array([1, 2, 3]) },
  ]);
  const decoded = decodeSetupPack(bytes);
  const payload = decoded.entries.get(0x02000001)!;
  assert.equal(payload.buffer, bytes.buffer);
  // Mutating the view mutates the backing buffer.
  payload[0] = 0xff;
  assert.equal(bytes[ACSP_HEADER_LEN + 12], 0xff);
});

test("decodes the spec worked example", () => {
  const bytes = bytesFromHex(`
    41 43 53 50 01 03 00 00  7F 1B 00 02  02 00 00 00
    01 00 00 02  28 00 00 00  02 00 00 00
    02 00 00 02  00 00 00 00  FF FF FF FF
    DE AD
  `);
  const decoded = decodeSetupPack(bytes);
  assert.equal(decoded.setupId, 0x02001b7f);
  assert.deepEqual(decoded.entries.get(0x02000001), new Uint8Array([0xde, 0xad]));
  assert.ok(decoded.entries.has(0x02000002));
  assert.equal(decoded.entries.get(0x02000002), undefined);
});

test("isSetupPack matches only ACSP v1 setup packs", () => {
  const bytes = encodeSetupPack(0x02001b7f, [{ id: 0x02000001, data: new Uint8Array([1]) }]);
  assert.equal(isSetupPack(bytes), true);
  assert.equal(isSetupPack(new Uint8Array(0)), false);
  assert.equal(isSetupPack(new Uint8Array(10)), false);

  let b = clone(bytes);
  b[0] = 0;
  assert.equal(isSetupPack(b), false);

  b = clone(bytes);
  b[4] = 2;
  assert.equal(isSetupPack(b), false);

  b = clone(bytes);
  b[5] = 1;
  assert.equal(isSetupPack(b), false);
});

test("rejects invalid packs", () => {
  const base = encodeSetupPack(0x02001b7f, [
    { id: 0x02000001, data: new Uint8Array([1, 2]) },
    { id: 0x02000002, data: new Uint8Array([3, 4, 5]) },
    { id: 0x02000003, data: undefined },
  ]);

  // 1. fewer than 16 bytes
  assert.throws(() => decodeSetupPack(new Uint8Array(10)), /too short/);

  // 2. bad magic
  let b = clone(base);
  b[0] = 0;
  assert.throws(() => decodeSetupPack(b), /bad magic/);

  // 3. version != 1
  b = clone(base);
  b[4] = 2;
  assert.throws(() => decodeSetupPack(b), /unsupported version/);

  // 4. kind != 3
  b = clone(base);
  b[5] = 2;
  assert.throws(() => decodeSetupPack(b), /invalid kind/);

  // 5. reserved != 0
  b = clone(base);
  b[6] = 1;
  assert.throws(() => decodeSetupPack(b), /reserved/);

  // 6. truncated entry table
  assert.throws(() => decodeSetupPack(base.slice(0, 20)), /truncated entry table/);

  // 7. present payload extends past EOF
  b = clone(base);
  new DataView(b.buffer).setUint32(ACSP_HEADER_LEN + 8, 100, true);
  assert.throws(() => decodeSetupPack(b), /out of bounds/);

  // 8. present payload offset points into the table
  b = clone(base);
  new DataView(b.buffer).setUint32(ACSP_HEADER_LEN + 4, 20, true);
  assert.throws(() => decodeSetupPack(b), /out of bounds/);

  // 9a. gap between payloads: entry 1 starts one byte late.
  b = clone(base);
  new DataView(b.buffer).setUint32(ACSP_HEADER_LEN + 12 + 4, 55, true);
  new DataView(b.buffer).setUint32(ACSP_HEADER_LEN + 12 + 8, 2, true);
  assert.throws(() => decodeSetupPack(b), /do not tile exactly/);

  // 9b. overlap: entry 1 starts one byte early.
  b = clone(base);
  new DataView(b.buffer).setUint32(ACSP_HEADER_LEN + 12 + 4, 53, true);
  new DataView(b.buffer).setUint32(ACSP_HEADER_LEN + 12 + 8, 3, true);
  assert.throws(() => decodeSetupPack(b), /do not tile exactly/);

  // 9c. trailing bytes
  b = new Uint8Array(base.byteLength + 1);
  b.set(base);
  assert.throws(() => decodeSetupPack(b), /trailing bytes/);

  // 10. duplicate ids
  b = clone(base);
  new DataView(b.buffer).setUint32(ACSP_HEADER_LEN + 12, 0x02000001, true);
  assert.throws(() => decodeSetupPack(b), /duplicate id/);
});

test("a missing entry with trailing bytes still fails tiling", () => {
  const base = encodeSetupPack(0x02001b7f, [{ id: 0x02000001, data: undefined }]);
  const withTrailing = new Uint8Array(base.byteLength + 1);
  withTrailing.set(base);
  assert.throws(() => decodeSetupPack(withTrailing), /trailing bytes/);
});
