import assert from "node:assert/strict";
import test from "node:test";
import { encodeAppearance, decodeAppearance, type Appearance } from "../src/appearance.js";
import { ACLI_HEADER_LEN, ACLI_RECORD_LEN, decodeAcli, encodeAcli, type LandblockInstance } from "../src/acli.js";

const appearance: Appearance = {
  paletteBaseId: 0x0400007e,
  parts: [{ partIndex: 0, gfxObjId: 0x01001234 }, { partIndex: 0, gfxObjId: 0x01005678 }],
  textures: [{ partIndex: 0, oldTextureId: 0x05000001, newTextureId: 0x05000002 },
    { partIndex: 0, oldTextureId: 0x05000003, newTextureId: 0x05000004 }],
  palettes: [{ paletteId: 0x04000001, offset: 1, length: 4 }, { paletteId: 0x04000002, offset: 2, length: 0 }],
};
const row: LandblockInstance = {
  setupId: 0x02000001, originX: 1, originY: 2, originZ: 3, quatW: 1, quatX: 0, quatY: 0, quatZ: 0,
  scale: 1, paletteId: 0, paletteOffset: 0, paletteLength: 0, weenieType: 10, flags: 2,
  motionTableId: 0x09000001, physicsScriptId: 0, guid: 0x70000001,
  textureOverrides: [{ partIndex: 3, newTextureId: 0x05000005 }],
};

test("resolved appearance binary retains ordered part/texture/range operations", () => {
  const bytes = encodeAppearance(appearance);
  assert.equal(bytes.length, 12 + 2 * 6 + 2 * 10 + 2 * 8);
  assert.deepEqual(decodeAppearance(bytes), appearance);
  const envelope = new Uint8Array(bytes.length + 7);
  envelope.set(bytes, 3);
  assert.deepEqual(decodeAppearance(envelope.subarray(3, 3 + bytes.length)), appearance);
});

test("appearance validates operation counts, truncation and integer ranges", () => {
  const bytes = encodeAppearance(appearance);
  for (let i = 0; i < bytes.length; i++) assert.throws(() => decodeAppearance(bytes.subarray(0, i)));
  const reserved = bytes.slice(); reserved[10] = 1;
  assert.throws(() => decodeAppearance(reserved, false), /reserved/);
  assert.throws(() => encodeAppearance({ ...appearance, parts: [{ partIndex: 0x10000, gfxObjId: 1 }] }), /out of range/);
  assert.throws(() => encodeAppearance({ ...appearance, paletteBaseId: NaN }), /out of range/);
});

test("ACLI mixed records round-trip appearance and retain placement identity/order", () => {
  const rows = [row, { ...row, guid: undefined, appearance }, { ...row, appearance: { parts: [], textures: [], palettes: [] } }];
  // GUID absence is represented by omission in the decoder.
  delete rows[1].guid;
  const bytes = encodeAcli(0x1234, rows);
  assert.equal(bytes[6], 1);
  assert.deepEqual(decodeAcli(bytes), { landblock: 0x1234, instances: rows });
  assert.deepEqual(encodeAcli(0x1234, decodeAcli(bytes).instances), bytes);
});

test("ACLI appearance is explicitly signalled and rejects corrupt tails without layout fallback", () => {
  const bytes = encodeAcli(0x1234, [{ ...row, textureOverrides: [], appearance }]);
  const tail = ACLI_HEADER_LEN + ACLI_RECORD_LEN;
  const corrupt = bytes.slice(); new DataView(corrupt.buffer).setUint32(tail, 0xffffffff, true);
  assert.throws(() => decodeAcli(corrupt), /truncated appearance/);
  for (let i = tail; i < bytes.length; i++) assert.throws(() => decodeAcli(bytes.subarray(0, i)));
  const invalid = bytes.slice(); invalid[tail + 4 + 10] = 1;
  assert.throws(() => decodeAcli(invalid), /reserved/);
});
