import assert from "node:assert/strict";
import test from "node:test";
import {
  ACVC_KIND_INSTANCE_INDEX,
  ACVC_VERSION,
  decodeInstanceIndex,
  encodeInstanceIndex,
  isInstanceIndexManifest,
  type InstanceIndexRow,
  type InstanceIndexManifest,
} from "../src/instanceIndex.js";

const row = (overrides: Partial<InstanceIndexRow> = {}): InstanceIndexRow => ({
  name: "Portal to Holtburg",
  blockId: 0x0125,
  setupId: 0x020001b3,
  weenieType: 7,
  motionTableId: 0x09000003,
  ordinal: 0,
  guid: 0,
  ...overrides,
});

test("ACVC round-trips names, ids, motion and ordinals", () => {
  const rows = [
    row(),
    row({ name: "Bronze Long Sword", setupId: 0x02000065, weenieType: 19, motionTableId: 0, ordinal: 0 }),
    row({ name: "Bronze Long Sword", setupId: 0x02000065, blockId: 0x0126, ordinal: 1 }),
    row({
      name: "Café Ünïcode – 名前",
      blockId: 0xffff,
      setupId: 0xffffffff,
      weenieType: 255,
      ordinal: 400,
      guid: 0x8000beef,
    }),
  ];
  const bytes = encodeInstanceIndex(rows);
  const index = decodeInstanceIndex(bytes);

  assert.equal(index.rowCount, rows.length);
  for (let i = 0; i < rows.length; i++) {
    assert.equal(index.name(i), rows[i].name);
    assert.equal(index.blockId[i], rows[i].blockId);
    assert.equal(index.setupId[i], rows[i].setupId);
    assert.equal(index.weenieType[i], rows[i].weenieType);
    assert.equal(index.motionTableId[i], rows[i].motionTableId);
    assert.equal(index.ordinal[i], rows[i].ordinal);
    assert.equal(index.guid[i], rows[i].guid);
  }
});

test("ACVC shares one dictionary entry per distinct name", () => {
  const bytes = encodeInstanceIndex([
    row(),
    row({ ordinal: 1 }),
    row({ ordinal: 2, name: "other" }),
  ]);
  const index = decodeInstanceIndex(bytes);
  assert.deepEqual(index.names, ["Portal to Holtburg", "other"]);
  assert.equal(index.nameId[0], index.nameId[1]);
  assert.notEqual(index.nameId[1], index.nameId[2]);
});

test("ACVC header carries magic, version and kind", () => {
  const bytes = encodeInstanceIndex([row()]);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  assert.equal(String.fromCharCode(...bytes.subarray(0, 4)), "ACVC");
  assert.equal(view.getUint8(4), ACVC_VERSION);
  assert.equal(view.getUint8(5), ACVC_KIND_INSTANCE_INDEX);
  // Sections are 4-byte aligned, so typed-array views need no copy.
  assert.equal(bytes.byteLength % 4, 0);
});

test("ACVC handles an empty index", () => {
  const index = decodeInstanceIndex(encodeInstanceIndex([]));
  assert.equal(index.rowCount, 0);
  assert.deepEqual(index.names, []);
});

test("the index decodes a version-1 file with no guid column", () => {
  // v1: same columns, no guid. Build one by decoding a v2 file's header and
  // rewriting the version + dropping the trailing column.
  const v2 = encodeInstanceIndex([row({ guid: 0x80000001 }), row({ ordinal: 1 })]);
  const guidSection = 4; // written last
  const v1 = v2.subarray(0, v2.byteLength - 0);
  const truncated = v1.slice(0, v1.byteLength - 4 * 2);
  const view = new DataView(truncated.buffer);
  view.setUint8(4, 1);
  const index = decodeInstanceIndex(truncated);
  assert.equal(index.rowCount, 2);
  assert.deepEqual([...index.guid], [0, 0]);
  assert.equal(guidSection, 4);
});

test("ACVC rejects malformed files instead of returning garbage", () => {
  const good = encodeInstanceIndex([row(), row({ ordinal: 1 })]);
  assert.throws(() => decodeInstanceIndex(good.subarray(0, 8)), /ACVC:/);
  assert.throws(() => decodeInstanceIndex(good.subarray(0, good.length - 4)), /ACVC:/);

  const badMagic = good.slice();
  badMagic[0] = 0x42;
  assert.throws(() => decodeInstanceIndex(badMagic), /bad magic/);

  const badVersion = good.slice();
  badVersion[4] = 99;
  assert.throws(() => decodeInstanceIndex(badVersion), /unsupported version/);

  const badKind = good.slice();
  badKind[5] = 99;
  assert.throws(() => decodeInstanceIndex(badKind), /not an instance index/);
});

test("ACVC encoder refuses a name dictionary past u16", () => {
  const rows = Array.from({ length: 70_000 }, (_, i) => row({ name: `name-${i}` }));
  assert.throws(() => encodeInstanceIndex(rows), /65,536 distinct names/);
});

test("the manifest is validated strictly", () => {
  const manifest: InstanceIndexManifest = {
    format: "ACVC",
    version: ACVC_VERSION,
    rows: 66_667,
    bytes: 1_000_003,
    file: "instance-index-0123456789abcdef.bin",
    hash: "0123456789abcdef",
    generatedAt: "2026-10-22T00:00:00.000Z",
  };
  assert.ok(isInstanceIndexManifest(manifest));
  assert.equal(isInstanceIndexManifest({ ...manifest, file: "index.bin" }), false);
  assert.equal(isInstanceIndexManifest({ ...manifest, version: 3 }), false);
  assert.equal(isInstanceIndexManifest({ ...manifest, rows: -1 }), false);
  assert.equal(isInstanceIndexManifest(null), false);
  assert.equal(isInstanceIndexManifest("ACVC"), false);
});
