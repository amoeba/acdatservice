import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";
import { decodeAppearance, encodeAppearance, isAppearance, MAX_ATTACHMENT_DEPTH, type Appearance, type HeldAttachment } from "../src/appearance.js";
import { decodeAcli, encodeAcli, type LandblockInstance } from "../src/acli.js";

const empty = (): Appearance => ({ parts: [], textures: [], palettes: [] });
const held = (): HeldAttachment => ({ setupId: 0x02000123, parentLocation: 3, placementId: 6, scale: 1.25 });
const row: LandblockInstance = { setupId: 0x02000001, originX: 1, originY: 2, originZ: 3,
  quatW: 1, quatX: 0, quatY: 0, quatZ: 0, scale: 1, paletteId: 0, paletteOffset: 0, paletteLength: 0,
  weenieType: 10, flags: 2, motionTableId: 0, physicsScriptId: 0, textureOverrides: [] };
const chain = (edges: number): Appearance => edges === 0 ? empty() : { ...empty(), attachments: [{ ...held(), appearance: chain(edges - 1) }] };

test("attachment appearance matches cross-language golden bytes and normalizes empty arrays", () => {
  const appearance = { ...empty(), attachments: [{ ...held(), appearance: { ...empty(), paletteBaseId: 0x0400007e } }] };
  const hex = "00000000000000000000010023010002030006000000a03f0c0000007e0000040000000000000000";
  assert.equal(Buffer.from(encodeAppearance(appearance)).toString("hex"), hex);
  assert.deepEqual(decodeAppearance(Buffer.from(hex,"hex")), appearance);
  assert.deepEqual(decodeAppearance(encodeAppearance({ ...empty(), attachments: [] })), empty());
  assert.throws(() => decodeAppearance(Buffer.from(hex,"hex"), false), /reserved/);
  assert.throws(() => encodeAppearance(appearance, false), /feature required/);
});

test("ACLI attachment feature is global across mixed rows; shared Rust golden is stable", () => {
  const nested: Appearance = { ...empty(), attachments: [{ ...held(), appearance: {
    ...empty(), paletteBaseId: 0x0400007e, attachments: [{ setupId: 0x02000456, parentLocation: 1, placementId: 1, scale: 0.5 }],
  } }] };
  const instances = [{ ...row, guid: 1 }, { ...row, guid: 2, appearance: empty() }, { ...row, guid: 3, appearance: nested }];
  const bytes = encodeAcli(0x1234, instances);
  assert.equal(bytes[6], 3);
  assert.deepEqual(decodeAcli(bytes), { landblock: 0x1234, instances });
  const golden = fs.readFileSync(new URL("../../tests/fixtures/acli-attachments.hex", import.meta.url), "utf8").trim();
  assert.equal(Buffer.from(bytes).toString("hex"), golden);
  assert.equal(encodeAcli(0, [{ ...row, appearance: empty() }])[6], 1);
  const only2 = bytes.slice(); only2[6] = 2;
  assert.throws(() => decodeAcli(only2), /requires appearance/);
  const unknown = bytes.slice(); unknown[6] = 7;
  assert.throws(() => decodeAcli(unknown), /unknown feature/);
  const downgrade = bytes.slice(); downgrade[6] = 1;
  assert.throws(() => decodeAcli(downgrade), /reserved/);
  // Every truncated prefix of an explicitly feature-bearing file fails. No
  // reinterpretation as a 52/56-byte legacy record may hide malformed tails.
  for (let i = 16; i < bytes.length; i++) assert.throws(() => decodeAcli(bytes.subarray(0,i)));
  assert.throws(() => decodeAcli(new Uint8Array([...bytes, 0])), /trailing/);
});

test("bounds cover sibling-wide nodes, leaf edges, cycles, and encoded malformed trees", () => {
  assert.deepEqual(decodeAppearance(encodeAppearance(chain(MAX_ATTACHMENT_DEPTH))), chain(MAX_ATTACHMENT_DEPTH));
  assert.throws(() => encodeAppearance(chain(MAX_ATTACHMENT_DEPTH + 1)), /depth/);
  const fifthLeaf = chain(4); let leaf = fifthLeaf;
  for (let i=0; i<4; i++) leaf = leaf.attachments![0].appearance!;
  leaf.attachments = [held()]; // depth5 forbidden even without an appearance
  assert.throws(() => encodeAppearance(fifthLeaf), /depth/);
  assert.doesNotThrow(() => encodeAppearance({ ...empty(), attachments: Array.from({length:32}, held) }));
  assert.throws(() => encodeAppearance({ ...empty(), attachments: [
    { ...held(), appearance: { ...empty(), attachments: Array.from({length:16}, held) } },
    { ...held(), appearance: { ...empty(), attachments: Array.from({length:16}, held) } },
  ] }), /too many/);
  const cycle = empty(); cycle.attachments = [{ ...held(), appearance: cycle }];
  assert.throws(() => encodeAppearance(cycle), /cycle/);
  assert.equal(isAppearance(cycle), false);
  assert.equal(isAppearance({ ...empty(), attachments: new Array(1) }), false);
  assert.equal(isAppearance({ ...empty(), attachments: [{ ...held(), appearance: null }] }), false);
  const bytes = encodeAppearance({ ...empty(), attachments: [held()] });
  for (const [offset, value] of [[10,33],[12,0x01000001],[16,4],[18,52],[24,0xffffffff]] as const) {
    const broken = bytes.slice(); const dv = new DataView(broken.buffer);
    if (offset === 10 || offset === 16 || offset === 18) dv.setUint16(offset,value,true); else dv.setUint32(offset,value,true);
    assert.throws(() => decodeAppearance(broken));
  }
  for (const scale of [0,-1,NaN,Infinity,Number.MIN_VALUE,Number.MAX_VALUE]) {
    assert.throws(() => encodeAppearance({ ...empty(), attachments: [{...held(),scale}] }), /scale/);
    const broken = bytes.slice(); new DataView(broken.buffer).setFloat32(20,scale,true);
    assert.throws(() => decodeAppearance(broken), /scale/);
  }
  // Build malformed over-depth bytes independently of the bounded encoder.
  let wire = encodeAppearance(empty());
  for (let i=0;i<5;i++) {
    const next = new Uint8Array(28+wire.length); const dv = new DataView(next.buffer);
    dv.setUint16(10,1,true); dv.setUint32(12,0x02000123,true); dv.setUint16(16,1,true); dv.setUint16(18,1,true);
    dv.setFloat32(20,1,true); dv.setUint32(24,wire.length,true); next.set(wire,28); wire=next;
  }
  assert.throws(() => decodeAppearance(wire), /depth/);
  // Two independently valid sibling subtrees must still share ONE node budget.
  const subtree=encodeAppearance({ ...empty(), attachments:Array.from({length:16},held) });
  const siblings=new Uint8Array(12+2*(16+subtree.length));const dv=new DataView(siblings.buffer);
  dv.setUint16(10,2,true);
  for(let at=12,i=0;i<2;i++,at+=16+subtree.length){
    dv.setUint32(at,0x02000123,true);dv.setUint16(at+4,1,true);dv.setUint16(at+6,1,true);dv.setFloat32(at+8,1,true);
    dv.setUint32(at+12,subtree.length,true);siblings.set(subtree,at+16);
  }
  assert.throws(()=>decodeAppearance(siblings),/too many/);
});

