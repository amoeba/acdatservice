import assert from "node:assert/strict";
import test from "node:test";
import { datFor, datObjectKey, parseDatName } from "../src/datRouting.js";

test("parseDatName accepts the service aliases", () => {
  assert.equal(parseDatName("portal"), "portal");
  assert.equal(parseDatName("client_portal.dat"), "portal");
  assert.equal(parseDatName("CELL"), "cell");
  assert.equal(parseDatName("client_cell.dat"), "cell");
  assert.equal(parseDatName("client_cell_1.dat"), "cell");
  assert.equal(parseDatName("HighRes"), "highres");
  assert.equal(parseDatName("client_highres.dat"), "highres");
  assert.equal(parseDatName("local-english"), "local_english");
  assert.equal(parseDatName("client_local_english.dat"), "local_english");
  assert.throws(() => parseDatName("world"), /Invalid DAT name/);
});

test("datObjectKey matches the service archive names", () => {
  assert.equal(datObjectKey("portal"), "client_portal.dat");
  assert.equal(datObjectKey("cell"), "client_cell_1.dat");
  assert.equal(datObjectKey("highres"), "client_highres.dat");
  assert.equal(datObjectKey("local_english"), "client_local_english.dat");
});

test("explicit archive and kind override the historical id fallback", () => {
  // 0x3300067a is a portal PhysicsScript id that the historical fallback
  // reports as cell because its low word is >= 0x0100.
  assert.equal(datFor(0x3300067a), "cell");
  assert.equal(datFor(0x3300067a, { archive: "portal" }), "portal");
  assert.equal(datFor(0x3300067a, { kind: "physicsScript" }), "portal");
  assert.equal(datFor(0xb34a0100, { kind: "envcell" }), "cell");
  assert.equal(datFor(0x02000001, { kind: "localEnglish" }), "local_english");
  assert.equal(datFor(0x06000001, { archive: "highres", kind: "texture" }), "highres");
});

test("historical fallback keeps the existing legacy routes", () => {
  assert.equal(datFor(0x01000001), "portal"); // GfxObj family
  assert.equal(datFor(0x02000346), "portal"); // Setup family
  assert.equal(datFor(0xb34a0100), "cell"); // indoor EnvCell shape
  assert.equal(datFor(0x0000ffff), "cell"); // CellLandblock
  assert.equal(datFor(0x0000fffe), "cell"); // LandblockInfo
  assert.equal(datFor(0x00000001), "portal"); // low suffix: legacy default
});
