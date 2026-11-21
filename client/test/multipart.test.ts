import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";
import { extractBoundary, parseMultipartMixed } from "../src/multipart.js";

const fixture = (name: string): Uint8Array =>
  new Uint8Array(readFileSync(new URL(`./fixtures/${name}`, import.meta.url)));

const enc = new TextEncoder();
const concat = (...parts: Uint8Array[]) => {
  const out = new Uint8Array(parts.reduce((n, part) => n + part.length, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
};

function part(boundary: string, id: string, filename: string, data: Uint8Array): Uint8Array {
  const header = "--" + boundary + "\r\nContent-Type: application/octet-stream\r\nContent-ID: <0x" + id +
    ">\r\nContent-Disposition: attachment; filename=\"" + filename +
    "\"\r\nContent-Location: /dats/portal/files/0x" + id + "\r\n\r\n";
  return concat(enc.encode(header), data, enc.encode("\r\n"));
}

function response(boundary: string, ...parts: Uint8Array[]): Uint8Array {
  return concat(...parts, enc.encode("--" + boundary + "--\r\n"));
}

test("extractBoundary handles quoted, unquoted, and missing parameters", () => {
  assert.equal(extractBoundary("multipart/mixed; boundary=abc123"), "abc123");
  assert.equal(extractBoundary('multipart/mixed; charset=utf-8; boundary="quoted-value"'), "quoted-value");
  assert.equal(extractBoundary("multipart/mixed"), null);
  assert.equal(extractBoundary(null), null);
});

test("parses service parts in order and preserves binary payload and metadata", () => {
  const boundary = "acdatservice-02000108-0";
  const setupBytes = new Uint8Array([0x00, 0xff, 0x0d, 0x0a, 0x2d, 0x2d, 0x6e, 0x6f, 0x74, 0x2d, 0x62, 0x6f, 0x75, 0x6e, 0x64, 0x61, 0x72, 0x79]);
  const gfxBytes = new Uint8Array([0x01, 0x02, 0x03]);
  const bytes = response(boundary,
    part(boundary, "02000108", "0x02000108.setup.bin", setupBytes),
    part(boundary, "010008AB", "0x010008AB.gfxobj.bin", gfxBytes),
  );
  const parsed = parseMultipartMixed(bytes, 'multipart/mixed; boundary="' + boundary + '"');
  assert.equal(parsed.length, 2);
  assert.deepEqual(parsed.map(p => p.fileId), [0x02000108, 0x010008ab]);
  assert.equal(parsed[0].contentType, "application/octet-stream");
  assert.equal(parsed[0].filename, "0x02000108.setup.bin");
  assert.equal(parsed[0].contentLocation, "/dats/portal/files/0x02000108");
  assert.deepEqual(parsed[0].body, setupBytes);
  assert.deepEqual(parsed[1].body, gfxBytes);
  assert.notEqual(parsed[0].body.buffer, bytes.buffer, "payload is returned as an owned copy");
});

test("accepts the legacy bare-boundary argument and skips parts without Content-ID", () => {
  const boundary = "legacy";
  const anonymous = concat(
    enc.encode("--" + boundary + "\r\nContent-Type: text/plain\r\n\r\nignored\r\n"),
    enc.encode("--" + boundary + "\r\nContent-ID: <0x00000000>\r\n\r\n\r\n"),
    enc.encode("--" + boundary + "--\r\n"),
  );
  const parsed = parseMultipartMixed(anonymous, boundary);
  assert.equal(parsed.length, 1);
  assert.equal(parsed[0].fileId, 0);
  assert.deepEqual(parsed[0].body, new Uint8Array());
  assert.equal(parsed[0].contentLocation, "");
});

test("rejects missing, malformed, or unterminated framing", () => {
  assert.throws(() => parseMultipartMixed(new Uint8Array(), undefined), /missing boundary/);
  assert.throws(() => parseMultipartMixed(new Uint8Array(), "multipart/mixed"), /missing boundary/);
  assert.throws(() => parseMultipartMixed(new Uint8Array(), "bad\r\nboundary"), /invalid boundary/);
  assert.throws(() => parseMultipartMixed(enc.encode("--x\r\n"), "x"), /missing header terminator/);
  assert.throws(() => parseMultipartMixed(enc.encode("--x\r\nA: b\r\n\r\ndata"), "x"), /unterminated part/);
});

test("malformed boundary-like payload bytes are not treated as delimiters", () => {
  const boundary = "collision-check";
  const payload = enc.encode("before\r\n--collision-checkX\r\nafter");
  const parsed = parseMultipartMixed(response(boundary, part(boundary, "00000001", "one.bin", payload)), boundary);
  assert.deepEqual(parsed[0].body, payload);
});

test("recorded /setups/0x02000108?include=gfxobjs response stays parseable", () => {
  const bytes = fixture("multipart-0x02000108.bin");
  assert.equal(bytes.byteLength, 4118);
  assert.equal(
    createHash("sha256").update(bytes).digest("hex"),
    "ed69b223e7eb7068f1e6d4b59c159791eebe39a4febdc1997dd1ff898c13b019",
  );

  const contentType = "multipart/mixed; boundary=acdatservice-02000108-0";
  assert.equal(extractBoundary(contentType), "acdatservice-02000108-0");
  const parts = parseMultipartMixed(bytes, contentType);
  assert.equal(parts.length, 2);

  const setup = parts.find((part) => part.fileId === 0x02000108);
  const gfx = parts.find((part) => part.fileId === 0x010008ab);
  assert.ok(setup);
  assert.ok(gfx);
  assert.equal(setup.contentType, "application/octet-stream");
  assert.equal(gfx.contentType, "application/octet-stream");
  assert.equal(setup.contentLocation, "/dats/portal/files/0x02000108");
  assert.equal(gfx.contentLocation, "/dats/portal/files/0x010008AB");

  // Independently slice each raw body: header terminator to the CRLF before the
  // next delimiter. This is the same recorded HTTP body libac used to test its
  // now-removed service-specific parser.
  const indexOf = (needle: string, from: number): number => {
    const encoded = enc.encode(needle);
    outer: for (let at = from; at <= bytes.length - encoded.length; at++) {
      for (let i = 0; i < encoded.length; i++) {
        if (bytes[at + i] !== encoded[i]) continue outer;
      }
      return at;
    }
    throw new Error(`fixture marker not found: ${needle}`);
  };
  const rawBodies = (() => {
    const boundary = "acdatservice-02000108-0";
    const bodies: Uint8Array[] = [];
    let at = 0;
    for (let i = 0; i < 2; i++) {
      const header = indexOf(`--${boundary}\r\n`, at);
      const dataStart = indexOf("\r\n\r\n", header) + 4;
      const next = indexOf(`\r\n--${boundary}`, dataStart);
      bodies.push(bytes.slice(dataStart, next));
      at = next + 2;
    }
    return bodies;
  })();
  assert.deepEqual(setup.body, rawBodies[0]);
  assert.deepEqual(gfx.body, rawBodies[1]);
});
