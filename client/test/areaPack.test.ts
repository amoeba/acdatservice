import assert from "node:assert/strict";
import test from "node:test";
import { areaFileKey, areaInstanceKey, decodeAreaPack, encodeAreaPack, type AreaPackMetadata } from "../src/areaPack.js";

const metadata: AreaPackMetadata = { areaId: "test-area", blocks: [2, 1], acliRevision: "rev_1" };
const enc = new TextEncoder();
const base = () => encodeAreaPack(metadata, [
  ["portal:00000001", new Uint8Array([1, 2])],
  ["cell:00000001", new Uint8Array(0)],
  ["instances:rev_1:0002", undefined],
]);
const view = (bytes: Uint8Array) => new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
function rewrite(bytes: Uint8Array, fn: (doc: any) => void): Uint8Array {
  const jsonLength = view(bytes).getUint32(8, true);
  const doc = JSON.parse(new TextDecoder().decode(bytes.subarray(16, 16 + jsonLength)));
  fn(doc);
  const json = enc.encode(JSON.stringify(doc));
  const out = new Uint8Array(16 + json.length + bytes.length - 16 - jsonLength);
  out.set(bytes.subarray(0, 16));
  new DataView(out.buffer).setUint32(8, json.length, true);
  out.set(json, 16);
  out.set(bytes.subarray(16 + jsonLength), 16 + json.length);
  return out;
}

test("area keys preserve DAT identity and ACLI revision", () => {
  assert.equal(areaFileKey(1, "portal"), "portal:00000001");
  assert.equal(areaFileKey(1, "cell"), "cell:00000001");
  assert.equal(areaFileKey(-1, "highres"), "highres:ffffffff");
  assert.equal(areaInstanceKey(0x10002, "rev_1"), "instances:rev_1:0002");
});

test("round trips missing, empty, and present entries and sorted metadata", () => {
  const decoded = decodeAreaPack(base());
  assert.deepEqual(decoded.metadata, { areaId: "test-area", blocks: [1, 2], acliRevision: "rev_1" });
  assert.deepEqual(decoded.entries.get("portal:00000001"), new Uint8Array([1, 2]));
  assert.equal(decoded.entries.get("instances:rev_1:0002"), undefined);
  assert.ok(decoded.entries.has("instances:rev_1:0002"));
  assert.equal(decoded.entries.get("cell:00000001")?.byteLength, 0);
  assert.equal(decoded.entries.has("portal:00000002"), false);
  const sorted = encodeAreaPack(metadata, [["portal:00000002", undefined], ["cell:00000001", undefined]]);
  const reverse = encodeAreaPack(metadata, [["cell:00000001", undefined], ["portal:00000002", undefined]]);
  assert.deepEqual(sorted, reverse);
});

test("decodes a nonzero-byteOffset buffer and returns payload views", () => {
  const pack = base();
  const wrapped = new Uint8Array(pack.length + 7);
  wrapped.set(pack, 3);
  const source = wrapped.subarray(3, 3 + pack.length);
  const result = decodeAreaPack(source);
  const payload = result.entries.get("portal:00000001")!;
  assert.equal(payload.buffer, source.buffer);
  payload[0] = 99;
  assert.equal(source[16 + view(source).getUint32(8, true) + 0], 99);
});

test("rejects malformed headers, JSON, indexes, ranges, counts, and metadata", () => {
  const original = base();
  assert.throws(() => decodeAreaPack(new Uint8Array(15)), /truncated header/);
  for (const [at, value, pattern] of [[0, 0, /magic/], [4, 2, /version/], [7, 1, /reserved/]] as const) {
    const b = original.slice(); b[at] = value; assert.throws(() => decodeAreaPack(b), pattern);
  }
  let b = original.slice(); view(b).setUint32(8, 0xffffffff, true); assert.throws(() => decodeAreaPack(b), /JSON exceeds/);
  b = original.slice(); view(b).setUint32(12, 100001, true); assert.throws(() => decodeAreaPack(b), /too many/);
  b = original.slice(); view(b).setUint32(12, 2, true); assert.throws(() => decodeAreaPack(b), /count mismatch/);
  b = rewrite(original, d => { d.index[2].offset = 999; }); assert.throws(() => decodeAreaPack(b), /out of bounds/);
  b = rewrite(original, d => { d.index[2].offset = 1; d.index[2].length = 1; }); assert.throws(() => decodeAreaPack(b), /tile exactly/);
  b = rewrite(original, d => { d.index[1].key = d.index[0].key; }); assert.throws(() => decodeAreaPack(b), /duplicate/);
  b = rewrite(original, d => { d.metadata.acliRevision = "other"; }); assert.throws(() => decodeAreaPack(b), /invalid key/);
  b = rewrite(original, d => { d.metadata.blocks = [1, 1]; }); assert.throws(() => decodeAreaPack(b), /duplicate block/);
  b = rewrite(original, d => { d.metadata.blocks = [65536]; }); assert.throws(() => decodeAreaPack(b), /out of bounds/);
  b = original.slice(); b[16] = 0xff; assert.throws(() => decodeAreaPack(b), /UTF-8 or JSON/);
});

test("rejects invalid encoder metadata, keys, and duplicate keys", () => {
  assert.throws(() => encodeAreaPack({ ...metadata, areaId: "Bad" }, []), /areaId/);
  assert.throws(() => encodeAreaPack({ ...metadata, acliRevision: "x".repeat(65) }, []), /revision/);
  assert.throws(() => encodeAreaPack({ ...metadata, blocks: [2, 2] }, []), /duplicate block/);
  assert.throws(() => encodeAreaPack(metadata, [["instances:other:0001", undefined]]), /invalid key/);
  assert.throws(() => encodeAreaPack(metadata, [["portal:00000001", undefined], ["portal:00000001", undefined]]), /duplicate key/);
});
