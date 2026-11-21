import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { encodeBundle, decodeBundle, type BundleKind } from "../src/aclb.js";
import { encodeSetupPack, decodeSetupPack } from "../src/acsp.js";

test("ACLB and ACSP share Rust byte fixtures, including empty and missing members", () => {
  const cases = JSON.parse(readFileSync(new URL("../../tests/fixtures/service-packs.json", import.meta.url), "utf8")) as Array<{
    format: string; kind?: BundleKind; landblock?: number; setupId?: number;
    entries: Array<{ id: number; data: number[] | null }>; hex: string;
  }>;
  for (const c of cases) {
    const expected = Uint8Array.from(c.hex.match(/../g)!, byte => parseInt(byte, 16));
    if (c.format === "ACLB") {
      const entries = c.entries.map(e => ({ id: e.id, data: Uint8Array.from(e.data!) }));
      assert.deepEqual(encodeBundle(c.kind!, c.landblock!, entries), expected);
      const decoded = decodeBundle(expected);
      assert.equal(decoded.kind, c.kind);
      assert.equal(decoded.landblock, c.landblock);
      for (const e of entries) assert.deepEqual(decoded.entries.get(e.id), e.data);
    } else {
      const entries = c.entries.map(e => ({ id: e.id, data: e.data === null ? undefined : Uint8Array.from(e.data) }));
      assert.deepEqual(encodeSetupPack(c.setupId!, entries), expected);
      const decoded = decodeSetupPack(expected);
      assert.equal(decoded.setupId, c.setupId);
      for (const e of entries) {
        assert.ok(decoded.entries.has(e.id));
        assert.deepEqual(decoded.entries.get(e.id), e.data);
      }
    }
  }
});
