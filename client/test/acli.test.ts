import assert from "node:assert/strict";
import test from "node:test";
import {
  ACLI_HEADER_LEN,
  ACLI_RECORD_LEN,
  ACLI_PREVIOUS_RECORD_LEN,
  ACLI_LEGACY_RECORD_LEN,
  ACLI_FLAG_LINK_CHILD,
  decodeAcli,
  encodeAcli,
  hasAcliMagic,
  isAcli,
  isLinkChild,
  type LandblockInstance,
} from "../src/acli.js";

const bytesFromHex = (hex: string): Uint8Array =>
  Uint8Array.from(hex.split(/\s+/).filter(Boolean).map((b) => Number.parseInt(b, 16)));

const clone = (bytes: Uint8Array): Uint8Array => new Uint8Array(bytes);

const instance = (over: Partial<LandblockInstance> = {}): LandblockInstance => ({
  setupId: 0x02000117,
  originX: 10,
  originY: -30,
  originZ: 0,
  quatW: 1,
  quatX: 0,
  quatY: 0,
  quatZ: 0,
  scale: 1,
  paletteId: 0,
  paletteOffset: 0,
  paletteLength: 0,
  weenieType: 13,
  flags: 0,
  motionTableId: 0,
  physicsScriptId: 0,
  textureOverrides: [],
  ...over,
});

test("round-trips records and texture overrides", () => {
  const source = [
    instance({ setupId: 0x0200007c, originX: -4.05, originY: -27.1675, originZ: 0.0125, weenieType: 20 }),
    instance({
      setupId: 0x02000117,
      originX: 10,
      quatW: -0.707107,
      quatZ: -0.707107,
      flags: ACLI_FLAG_LINK_CHILD,
      scale: 1.5,
      paletteId: 0x040000ef,
      paletteOffset: 2,
      paletteLength: 8,
      motionTableId: 0x09000016,
      physicsScriptId: 0x3300067a,
      textureOverrides: [
        { partIndex: 3, newTextureId: 0x050018e6 },
        { partIndex: 6, newTextureId: 0x050018e7 },
      ],
    }),
  ];
  const bytes = encodeAcli(0x0125, source);
  const decoded = decodeAcli(bytes);
  assert.equal(decoded.landblock, 0x0125);
  assert.equal(decoded.instances.length, 2);
  // Byte-for-byte round-trip (floats are stored as f32, so compare bytes, not
  // the original f64 inputs).
  assert.deepEqual(encodeAcli(decoded.landblock, decoded.instances), bytes);
  assert.equal(decoded.instances[0].setupId, 0x0200007c);
  assert.equal(decoded.instances[0].weenieType, 20);
  assert.equal(decoded.instances[0].motionTableId, 0);
  assert.equal(decoded.instances[0].physicsScriptId, 0);
  assert.ok(Math.abs(decoded.instances[0].originX - -4.05) < 1e-5);
  assert.ok(Math.abs(decoded.instances[0].originY - -27.1675) < 1e-4);
  assert.equal(decoded.instances[1].flags, ACLI_FLAG_LINK_CHILD);
  assert.equal(decoded.instances[1].scale, 1.5);
  assert.equal(decoded.instances[1].motionTableId, 0x09000016);
  assert.equal(decoded.instances[1].physicsScriptId, 0x3300067a);
  assert.equal(decoded.instances[1].paletteId, 0x040000ef);
  assert.equal(decoded.instances[1].paletteOffset, 2);
  assert.equal(decoded.instances[1].paletteLength, 8);
  assert.deepEqual(decoded.instances[1].textureOverrides, [
    { partIndex: 3, newTextureId: 0x050018e6 },
    { partIndex: 6, newTextureId: 0x050018e7 },
  ]);
  assert.equal(isLinkChild(decoded.instances[1]), true);
  assert.equal(isLinkChild(decoded.instances[0]), false);
});

test("count=0 is a valid 16-byte file", () => {
  const bytes = encodeAcli(0x00ae, []);
  assert.equal(bytes.byteLength, ACLI_HEADER_LEN);
  const decoded = decodeAcli(bytes);
  assert.equal(decoded.landblock, 0x00ae);
  assert.deepEqual(decoded.instances, []);
});

test("decodes a hand-written record with physicsScriptId at offset 0x34", () => {
  // Header + one record: motionTableId at 0x30, physicsScriptId at 0x34,
  // then a texture override beginning at 0x38.
  const bytes = bytesFromHex(`
    41 43 4C 49 01 01 00 00 25 01 00 00 01 00 00 00
    7C 00 00 02 00 00 20 C1 CD CC D4 C1 4D 1C 4D 3C
    00 00 80 3F 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 80 3F 00 00 00 00 00 00 00 00 14 00 01 00
    16 00 00 09 7A 06 00 33 03 00 E6 18 00 05
  `);
  assert.equal(hasAcliMagic(bytes), true);
  assert.equal(isAcli(bytes), true);
  const decoded = decodeAcli(bytes);
  assert.equal(decoded.landblock, 0x0125);
  assert.equal(decoded.instances.length, 1);
  assert.equal(decoded.instances[0].setupId, 0x0200007c);
  assert.equal(decoded.instances[0].weenieType, 20);
  assert.equal(decoded.instances[0].motionTableId, 0x09000016);
  assert.equal(decoded.instances[0].physicsScriptId, 0x3300067a);
  assert.deepEqual(decoded.instances[0].textureOverrides, [
    { partIndex: 3, newTextureId: 0x050018e6 },
  ]);
});

test("decodes a legacy 52-byte record with no physicsScriptId", () => {
  // Pre-change layout, still present in the live R2 tree: header + one 52-byte
  // record ending in motionTableId at 0x30.
  const bytes = bytesFromHex(`
    41 43 4C 49 01 01 00 00 25 01 00 00 01 00 00 00
    7C 00 00 02 00 00 20 C1 CD CC D4 C1 4D 1C 4D 3C
    00 00 80 3F 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 80 3F 00 00 00 00 00 00 00 00 14 00 00 00
    16 00 00 09
  `);
  const decoded = decodeAcli(bytes);
  assert.equal(decoded.landblock, 0x0125);
  assert.equal(decoded.instances.length, 1);
  assert.equal(decoded.instances[0].motionTableId, 0x09000016);
  assert.equal(decoded.instances[0].physicsScriptId, 0);
  assert.deepEqual(decoded.instances[0].textureOverrides, []);
});

/** Pre-physicsScriptId encoder used only to test live-R2 compatibility. */
function encodeLegacyAcli(
  landblock: number,
  instances: readonly LandblockInstance[],
): Uint8Array {
  const header = 16;
  const record = 52;
  let size = header;
  for (const instance of instances) size += record + instance.textureOverrides.length * 6;
  const out = new Uint8Array(size);
  const dv = new DataView(out.buffer);
  out.set([0x41, 0x43, 0x4c, 0x49, 1, 1, 0, 0]);
  dv.setUint32(8, landblock, true);
  dv.setUint32(12, instances.length, true);
  let at = header;
  for (const instance of instances) {
    dv.setUint32(at, instance.setupId, true);
    dv.setFloat32(at + 4, instance.originX, true);
    dv.setFloat32(at + 8, instance.originY, true);
    dv.setFloat32(at + 12, instance.originZ, true);
    dv.setFloat32(at + 16, instance.quatW, true);
    dv.setFloat32(at + 20, instance.quatX, true);
    dv.setFloat32(at + 24, instance.quatY, true);
    dv.setFloat32(at + 28, instance.quatZ, true);
    dv.setFloat32(at + 32, instance.scale, true);
    dv.setUint32(at + 36, instance.paletteId, true);
    dv.setUint16(at + 40, instance.paletteOffset, true);
    dv.setUint16(at + 42, instance.paletteLength, true);
    dv.setUint8(at + 44, instance.weenieType);
    dv.setUint8(at + 45, instance.flags);
    dv.setUint16(at + 46, instance.textureOverrides.length, true);
    dv.setUint32(at + 48, instance.motionTableId, true);
    at += record;
    for (const override of instance.textureOverrides) {
      dv.setUint16(at, override.partIndex, true);
      dv.setUint32(at + 2, override.newTextureId, true);
      at += 6;
    }
  }
  return out;
}

test("decodes legacy multi-record files with texture overrides", () => {
  const source = [
    instance({ setupId: 0x0200007c, motionTableId: 0x09000004 }),
    instance({
      setupId: 0x020001b3,
      motionTableId: 0x09000003,
      textureOverrides: [{ partIndex: 2, newTextureId: 0x05001234 }],
    }),
  ];
  const decoded = decodeAcli(encodeLegacyAcli(0x0125, source));
  assert.equal(decoded.instances.length, 2);
  assert.equal(decoded.instances[0].motionTableId, 0x09000004);
  assert.equal(decoded.instances[1].motionTableId, 0x09000003);
  assert.equal(decoded.instances[1].physicsScriptId, 0);
  assert.deepEqual(decoded.instances[1].textureOverrides, [
    { partIndex: 2, newTextureId: 0x05001234 },
  ]);
});

/** The published 56-byte layout: physicsScriptId, no guid. */
function encodePreviousAcli(
  landblock: number,
  instances: readonly LandblockInstance[],
): Uint8Array {
  const out = encodeAcli(landblock, instances);
  // Drop the trailing guid (4 bytes) from each record and fix up the length.
  const record = ACLI_RECORD_LEN;
  const previous = new Uint8Array(
    ACLI_HEADER_LEN +
      instances.reduce((sum, i) => sum + ACLI_PREVIOUS_RECORD_LEN + i.textureOverrides.length * 6, 0),
  );
  previous.set(out.subarray(0, ACLI_HEADER_LEN));
  let from = ACLI_HEADER_LEN;
  let to = ACLI_HEADER_LEN;
  for (const instance of instances) {
    const overrides = instance.textureOverrides.length * 6;
    previous.set(out.subarray(from, from + ACLI_PREVIOUS_RECORD_LEN), to);
    previous.set(out.subarray(from + record, from + record + overrides), to + ACLI_PREVIOUS_RECORD_LEN);
    from += record + overrides;
    to += ACLI_PREVIOUS_RECORD_LEN + overrides;
  }
  return previous;
}

/** Decode-then-size check that mimics a pre-physicsScriptId client decoder. */
function legacyLayoutAccepts(bytes: Uint8Array): boolean {
  if (bytes.byteLength < 16) return false;
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const count = dv.getUint32(12, true);
  let at = 16;
  for (let i = 0; i < count; i++) {
    if (at + ACLI_LEGACY_RECORD_LEN > bytes.byteLength) return false;
    const textureCount = dv.getUint16(at + 46, true);
    at += ACLI_LEGACY_RECORD_LEN;
    if (at + textureCount * 6 > bytes.byteLength) return false;
    at += textureCount * 6;
  }
  return at === bytes.byteLength;
}

test("compatibility matrix: current 60-byte files are not readable by legacy layout assumptions", () => {
  const source = [
    instance({ physicsScriptId: 0x3300067a }),
    instance({
      setupId: 0x020001b3,
      motionTableId: 0x09000003,
      physicsScriptId: 0x330006da,
      textureOverrides: [{ partIndex: 2, newTextureId: 0x05001234 }],
    }),
  ];
  const current = encodeAcli(0x0125, source);
  assert.equal(current.byteLength, ACLI_HEADER_LEN + 2 * ACLI_RECORD_LEN + 6);
  assert.equal(legacyLayoutAccepts(current), false);
  assert.equal(decodeAcli(current).instances[1].physicsScriptId, 0x330006da);
});

test("compatibility matrix: current decoder reads 60, 56 and 52-byte trees", () => {
  const source = [instance({ motionTableId: 0x09000004, guid: 0x8000000a })];
  const current = encodeAcli(0x0125, source);
  const previous = encodePreviousAcli(0x0125, source);
  const legacy = encodeLegacyAcli(0x0125, source);
  assert.equal(current.byteLength, ACLI_HEADER_LEN + ACLI_RECORD_LEN);
  assert.equal(previous.byteLength, ACLI_HEADER_LEN + ACLI_PREVIOUS_RECORD_LEN);
  assert.equal(legacy.byteLength, ACLI_HEADER_LEN + ACLI_LEGACY_RECORD_LEN);
  assert.equal(legacyLayoutAccepts(legacy), true);
  assert.equal(decodeAcli(current).instances[0].motionTableId, 0x09000004);
  assert.equal(decodeAcli(previous).instances[0].motionTableId, 0x09000004);
  assert.equal(decodeAcli(legacy).instances[0].motionTableId, 0x09000004);
  assert.equal(decodeAcli(legacy).instances[0].physicsScriptId, 0);
});

test("guid round-trips and older layouts report it as absent", () => {
  const source = [instance({ guid: 0x8000beef, physicsScriptId: 0x3300067a })];
  const current = decodeAcli(encodeAcli(0x0125, source));
  assert.equal(current.instances[0].guid, 0x8000beef);
  assert.equal(current.instances[0].physicsScriptId, 0x3300067a);

  const previous = decodeAcli(encodePreviousAcli(0x0125, source));
  assert.equal(previous.instances[0].guid, undefined);
  assert.equal(previous.instances[0].physicsScriptId, 0x3300067a);

  const legacy = decodeAcli(encodeLegacyAcli(0x0125, source));
  assert.equal(legacy.instances[0].guid, undefined);
  assert.equal(legacy.instances[0].physicsScriptId, 0);
});

test("a record without a guid encodes as zero", () => {
  const bytes = encodeAcli(0x0125, [instance()]);
  const dv = new DataView(bytes.buffer);
  assert.equal(dv.getUint32(ACLI_HEADER_LEN + 56, true), 0);
});

test("rejects a file shorter than the header", () => {
  assert.throws(() => decodeAcli(new Uint8Array(10)), /too short/);
  assert.equal(hasAcliMagic(new Uint8Array(10)), false);
  assert.equal(isAcli(new Uint8Array(10)), false);
});

test("rejects bad magic / version / kind / reserved / landblock", () => {
  const base = encodeAcli(0x00ae, [instance()]);

  let b = clone(base);
  b[0] = 0;
  assert.throws(() => decodeAcli(b), /bad magic/);
  assert.equal(isAcli(b), false);

  b = clone(base);
  b[4] = 3;
  assert.throws(() => decodeAcli(b), /unsupported version/);
  assert.equal(isAcli(b), false);

  b = clone(base);
  b[5] = 2;
  assert.throws(() => decodeAcli(b), /invalid kind/);
  assert.equal(isAcli(b), false);

  b = clone(base);
  b[6] = 4; // bits 0 and 1 now declare appearance and attachment tails
  assert.throws(() => decodeAcli(b), /reserved/);

  b = clone(base);
  new DataView(b.buffer).setUint32(8, 0x1_0000, true);
  assert.throws(() => decodeAcli(b), /landblock out of range/);
});

test("rejects truncated records and overrides", () => {
  const base = encodeAcli(0x00ae, [instance()]);
  assert.equal(ACLI_RECORD_LEN, 60);
  assert.equal(base.byteLength, ACLI_HEADER_LEN + ACLI_RECORD_LEN);
  assert.throws(
    () => decodeAcli(base.slice(0, ACLI_HEADER_LEN + ACLI_RECORD_LEN - 1)),
    /truncated record/,
  );

  const withTextures = encodeAcli(0x00ae, [
    instance({ textureOverrides: [{ partIndex: 1, newTextureId: 0x05000001 }] }),
  ]);
  assert.throws(
    () => decodeAcli(withTextures.slice(0, withTextures.byteLength - 1)),
    /truncated texture overrides/,
  );
});

test("rejects trailing bytes", () => {
  const base = encodeAcli(0x00ae, [instance()]);
  const withTrailing = new Uint8Array(base.byteLength + 1);
  withTrailing.set(base);
  assert.throws(() => decodeAcli(withTrailing), /trailing bytes/);

  const empty = encodeAcli(0x00ae, []);
  const emptyTrailing = new Uint8Array(empty.byteLength + 1);
  emptyTrailing.set(empty);
  assert.throws(() => decodeAcli(emptyTrailing), /trailing bytes/);
});

test("encode validates landblock and override count", () => {
  assert.throws(() => encodeAcli(0x1_0000, []), /landblock out of range/);
  assert.throws(() => encodeAcli(-1, []), /landblock out of range/);
});
