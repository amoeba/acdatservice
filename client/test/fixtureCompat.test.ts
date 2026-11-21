import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  ACLI_FEATURE_APPEARANCE,
  decodeAcli,
  encodeAcli,
  type LandblockInstance,
} from "../src/acli.js";
import { decodeAppearance, encodeAppearance } from "../src/appearance.js";

const fixture = (name: string): Uint8Array => {
  const hex = readFileSync(new URL(`../../tests/fixtures/${name}`, import.meta.url), "utf8");
  return Uint8Array.from(hex.replace(/\s+/g, "").match(/../g) ?? [], (byte) =>
    Number.parseInt(byte, 16),
  );
};

const jsonFixture = <T>(name: string): T =>
  JSON.parse(readFileSync(new URL(`../../tests/fixtures/${name}`, import.meta.url), "utf8")) as T;

/**
 * These files are the Rust service's contract fixtures. The Rust codecs decode
 * them and re-encode the exact same bytes in
 * `tests/acli_appearance.rs` / `tests/acli_attachments.rs`; the TypeScript
 * package must agree byte-for-byte in both directions.
 */
test("Rust ACLI appearance fixture decodes and re-encodes byte-for-byte", () => {
  const bytes = fixture("acli-appearance.hex");
  const value = decodeAcli(bytes);
  assert.equal(bytes.byteLength, 262);
  assert.equal(value.landblock, 0x0125);
  assert.equal(value.instances.length, 3);
  assert.equal(value.instances[0].guid, 0x80000001);
  assert.equal(value.instances[0].textureOverrides.length, 1);
  assert.equal(value.instances[0].textureOverrides[0].newTextureId, 0x05000003);

  const appearance = value.instances[0].appearance;
  assert.ok(appearance);
  assert.equal(appearance.paletteBaseId, 0x04000001);
  assert.equal(appearance.parts[0].gfxObjId, 0x01001234);
  assert.equal(appearance.textures[0].oldTextureId, 0x05000001);
  assert.equal(appearance.textures[0].newTextureId, 0x05000002);
  assert.equal(appearance.palettes[0].offset, 2);
  assert.equal(appearance.palettes[0].length, 4);
  assert.equal(value.instances[1].appearance, undefined);
  assert.deepEqual(value.instances[2].appearance, { parts: [], textures: [], palettes: [] });
  assert.equal((bytes[6] ?? 0) & ACLI_FEATURE_APPEARANCE, ACLI_FEATURE_APPEARANCE);
  assert.deepEqual(encodeAcli(value.landblock, value.instances), bytes);
});

test("Rust ACLI attachment JSON encodes to the Rust attachment fixture", () => {
  const expected = fixture("acli-attachments.hex");
  const source = jsonFixture<{ landblock: number; instances: LandblockInstance[] }>(
    "acli-attachments.json",
  );
  const encoded = encodeAcli(source.landblock, source.instances);
  assert.deepEqual(encoded, expected);

  const decoded = decodeAcli(expected);
  assert.equal(decoded.landblock, source.landblock);
  assert.equal(decoded.instances.length, 3);
  assert.equal(decoded.instances[0].appearance, undefined);
  assert.deepEqual(decoded.instances[1].appearance, { parts: [], textures: [], palettes: [] });

  const outer = decoded.instances[2].appearance;
  assert.ok(outer?.attachments);
  assert.equal(outer.attachments.length, 1);
  const child = outer.attachments[0];
  assert.equal(child.setupId, 0x02000123);
  assert.equal(child.parentLocation, 3);
  assert.equal(child.placementId, 6);
  assert.equal(child.scale, 1.25);
  assert.equal(child.appearance?.paletteBaseId, 0x0400007e);
  assert.deepEqual(child.appearance?.attachments, [
    {
      setupId: 0x02000456,
      parentLocation: 1,
      placementId: 1,
      scale: 0.5,
    },
  ]);
  assert.deepEqual(encodeAcli(decoded.landblock, decoded.instances), expected);
});

test("standalone appearance codec matches the fixture payload exactly", () => {
  const value = decodeAcli(fixture("acli-appearance.hex")).instances[0].appearance;
  assert.ok(value);
  const encoded = encodeAppearance(value);
  // Header + one part + one texture + one palette = 12 + 6 + 10 + 8.
  assert.equal(encoded.byteLength, 36);
  assert.deepEqual(decodeAppearance(encoded), value);
});
