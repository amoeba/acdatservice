import assert from "node:assert/strict";
import test from "node:test";
import { encodeMultipartMixed, parseMultipartMixed } from "../src/multipart.js";

test("encoder preserves binary bytes, metadata, subviews, empty parts and ownership", () => {
  const bytes = Uint8Array.from({ length: 260 }, (_, i) => i & 255);
  const source = bytes.subarray(2, 258);
  const encoded = encodeMultipartMixed([
    { fileId: 0x06000001, body: source, contentType: "application/octet-stream", filename: 'a"b\\c.bin', contentLocation: "/dats/portal/files/0x06000001" },
    { fileId: 0x02000108, body: new Uint8Array() },
  ], "fixture_boundary");
  const decoded = parseMultipartMixed(encoded.body, encoded.contentType);
  assert.equal(decoded.length, 2);
  assert.deepEqual(decoded[0].body, source);
  assert.equal(decoded[0].filename, 'a"b\\c.bin');
  assert.equal(decoded[0].contentLocation, "/dats/portal/files/0x06000001");
  assert.equal(decoded[1].fileId, 0x02000108);
  assert.equal(decoded[1].filename, "0x02000108.bin");
  assert.equal(decoded[1].body.length, 0);
  encoded.body.fill(0);
  assert.deepEqual(decoded[0].body, source, "decoded payload owns its bytes");
  const empty = encodeMultipartMixed([]);
  assert.deepEqual(parseMultipartMixed(empty.body, empty.contentType), []);
});

test("encoder rejects framing/header injection, invalid ids and boundary collisions", () => {
  assert.throws(() => encodeMultipartMixed([{ fileId: -1, body: new Uint8Array() }]), /uint32/);
  assert.throws(() => encodeMultipartMixed([{ fileId: 1, body: new Uint8Array(), filename: "bad\r\nInjected: yes" }]), /header/);
  assert.throws(() => encodeMultipartMixed([], "bad\r\nboundary"), /boundary/);
  assert.throws(() => encodeMultipartMixed([{ fileId: 1, body: new TextEncoder().encode("\r\n--collision\r\n") }], "collision"), /occurs/);
});
