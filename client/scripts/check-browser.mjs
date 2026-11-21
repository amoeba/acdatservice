#!/usr/bin/env node
// Browser import/bundle smoke for @amoeba/acdatservice-client.
//
// Bundles the root and /multipart entry points for the browser with esbuild,
// then executes them in a bare vm context that intentionally has no Node, Three,
// or test-runner globals. The codecs must work using only web-standard data
// types (Uint8Array/DataView/TextEncoder/TextDecoder).

import assert from "node:assert/strict";
import { build } from "esbuild";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const entries = [
  { entry: "client/src/index.ts", global: "AcDatService" },
  { entry: "client/src/multipart.ts", global: "AcDatServiceMultipart" },
];

const FORBIDDEN = [
  "process",
  "Buffer",
  "require",
  "module",
  "exports",
  "__dirname",
  "__filename",
  "THREE",
  "jest",
  "describe",
  "it",
  "test",
  "expect",
];

async function bundle(entry, global) {
  const result = await build({
    absWorkingDir: root,
    entryPoints: [entry],
    bundle: true,
    write: false,
    format: "iife",
    globalName: global,
    platform: "browser",
    target: ["es2022"],
    logLevel: "silent",
  });
  return result.outputFiles[0].text;
}

function browserContext() {
  const sandbox = {
    // Host web-standard constructors keep typed-array identity stable across
    // the vm realm boundary; none of them is a Node-only API.
    ArrayBuffer,
    DataView,
    Map,
    Set,
    TextEncoder,
    TextDecoder,
    Uint8Array,
    Uint16Array,
    Uint32Array,
    console: { error: () => {}, log: () => {}, warn: () => {} },
  };
  const context = vm.createContext(sandbox);
  for (const name of FORBIDDEN) {
    assert.equal(vm.runInContext(`typeof ${name}`, context), "undefined", `${name} leaked into the browser context`);
  }
  return { sandbox, context };
}

function load(code, global) {
  const { sandbox, context } = browserContext();
  vm.runInContext(code, context, { filename: `${global}.browser.js` });
  assert.ok(sandbox[global], `${global} bundle did not initialize`);
  return sandbox[global];
}

const [rootBundle, multipartBundle] = await Promise.all(
  entries.map(({ entry, global }) => bundle(entry, global)),
);

const rootApi = load(rootBundle, entries[0].global);
const multipartApi = load(multipartBundle, entries[1].global);

for (const api of [rootApi, multipartApi]) {
  assert.equal(typeof api.parseMultipartMixed, "function");
  assert.equal(typeof api.extractBoundary, "function");
  assert.equal(typeof api.encodeMultipartMixed, "function");
}

const boundary = "browser-smoke";
const encodedPart = new TextEncoder().encode(
  `--${boundary}\r\nContent-ID: <0x02000001>\r\nContent-Type: application/octet-stream\r\n` +
    `Content-Disposition: attachment; filename="0x02000001.bin"\r\n\r\n`,
);
const payload = new Uint8Array([0x00, 0xff, 0x0d, 0x0a]);
const closing = new TextEncoder().encode(`\r\n--${boundary}--\r\n`);
const body = new Uint8Array(encodedPart.length + payload.length + closing.length);
body.set(encodedPart, 0);
body.set(payload, encodedPart.length);
body.set(closing, encodedPart.length + payload.length);
for (const api of [rootApi, multipartApi]) {
  const parts = api.parseMultipartMixed(body, `multipart/mixed; boundary=${boundary}`);
  assert.equal(parts.length, 1);
  assert.equal(parts[0].fileId, 0x02000001);
  assert.equal(parts[0].filename, "0x02000001.bin");
  assert.deepEqual(parts[0].body, payload);
  const encoded = api.encodeMultipartMixed([{ fileId: 0x02000001, body: payload }]);
  assert.deepEqual(api.parseMultipartMixed(encoded.body, encoded.contentType)[0].body, payload);
}

assert.equal(rootApi.datFor(0x3300067a, { archive: "portal" }), "portal");
assert.equal(rootApi.parseDatName("client_cell_1.dat"), "cell");

const bundleBytes = rootApi.encodeBundle(rootApi.BundleKind.Landblock, 0x00ae, [
  { id: 0x00ae0100, data: new Uint8Array([1, 2, 3]) },
]);
assert.deepEqual(rootApi.decodeBundle(bundleBytes).entries.get(0x00ae0100), new Uint8Array([1, 2, 3]));

const setupBytes = rootApi.encodeSetupPack(0x02000001, [
  { id: 0x02000001, data: new Uint8Array([4, 5]) },
  { id: 0x02000002, data: undefined },
]);
const setup = rootApi.decodeSetupPack(setupBytes);
assert.deepEqual(setup.entries.get(0x02000001), new Uint8Array([4, 5]));
assert.equal(setup.entries.get(0x02000002), undefined);

const appearance = {
  paletteBaseId: 0x04000001,
  parts: [{ partIndex: 0, gfxObjId: 0x01000001 }],
  textures: [{ partIndex: 0, oldTextureId: 0x05000001, newTextureId: 0x05000002 }],
  palettes: [{ paletteId: 0x04000002, offset: 1, length: 2 }],
};
const decodedAppearance = rootApi.decodeAppearance(rootApi.encodeAppearance(appearance));
assert.equal(decodedAppearance.paletteBaseId, appearance.paletteBaseId);
assert.equal(decodedAppearance.parts[0].gfxObjId, appearance.parts[0].gfxObjId);
assert.equal(decodedAppearance.textures[0].newTextureId, appearance.textures[0].newTextureId);
assert.equal(decodedAppearance.palettes[0].length, appearance.palettes[0].length);

const acliBytes = rootApi.encodeAcli(0x0125, [
  {
    setupId: 0x02000001,
    originX: 1,
    originY: 2,
    originZ: 3,
    quatW: 1,
    quatX: 0,
    quatY: 0,
    quatZ: 0,
    scale: 1,
    paletteId: 0,
    paletteOffset: 0,
    paletteLength: 0,
    weenieType: 10,
    flags: rootApi.ACLI_FLAG_NO_DRAW,
    motionTableId: 0,
    physicsScriptId: 0,
    textureOverrides: [],
    appearance,
  },
]);
const acli = rootApi.decodeAcli(acliBytes);
assert.equal(acli.instances[0].flags, 0x04);
assert.equal(acli.instances[0].appearance.paletteBaseId, 0x04000001);

const indexBytes = rootApi.encodeInstanceIndex([
  {
    name: "Browser smoke",
    blockId: 0x0125,
    setupId: 0x02000001,
    weenieType: 10,
    motionTableId: 0,
    ordinal: 0,
    guid: 1,
  },
]);
assert.equal(rootApi.decodeInstanceIndex(indexBytes).name(0), "Browser smoke");

const areaBytes = rootApi.encodeAreaPack(
  { areaId: "browser-smoke", blocks: [0x0125], acliRevision: "rev_1" },
  [["portal:02000001", new Uint8Array([9])]],
);
assert.deepEqual(rootApi.decodeAreaPack(areaBytes).entries.get("portal:02000001"), new Uint8Array([9]));

assert.equal(rootApi.currentAcliLayoutMarker().format, "ac-noclip/acli-layout");

console.log(
  `check-browser: OK — root (${rootBundle.length} B) and /multipart (${multipartBundle.length} B) ` +
    `bundles executed with no Node/Three/test globals.`,
);
