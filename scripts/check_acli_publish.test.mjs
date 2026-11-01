import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const script = new URL("./check_acli_publish.mjs", import.meta.url).pathname;
const golden = () => Buffer.from(fs.readFileSync(new URL("../tests/fixtures/acli-appearance.hex", import.meta.url), "utf8").trim(), "hex");

async function fixture(t) {
  const src = fs.mkdtempSync(path.join(os.tmpdir(), "acli-feature-guard-"));
  t.after(() => fs.rmSync(src, { recursive: true, force: true }));
  let marker = { format: "ac-noclip/acli-layout", version: 1, kind: 1,
    recordBaseBytes: 60, decoderRecordBaseBytes: [60], decoderFeatureBits: 1 };
  const server = http.createServer((_req, res) => {
    res.setHeader("content-type", "application/json"); res.end(JSON.stringify(marker));
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  t.after(() => new Promise(resolve => server.close(resolve)));
  const url = `http://127.0.0.1:${server.address().port}/acli-layout.json`;
  const value = {
    src,
    setMarker: value => marker = value,
    get marker() { return marker; },
    write: (bytes, name = "0x0125.bin") => fs.writeFileSync(path.join(src, name), bytes),
    run: () => new Promise(resolve => {
      const child = spawn(process.execPath, [script, "--src", src, "--client-url", url]);
      let output = "";
      child.stdout.on("data", bytes => output += bytes);
      child.stderr.on("data", bytes => output += bytes);
      child.on("close", code => resolve({ code, output }));
    }),
  };
  value.write(golden());
  return value;
}

function plain(block = 0x125) {
  const bytes = Buffer.alloc(16 + 60);
  bytes.write("ACLI"); bytes[4] = 1; bytes[5] = 1;
  bytes.writeUInt32LE(block, 8); bytes.writeUInt32LE(1, 12);
  return bytes;
}

test("accepts TS appearance golden with matching decoder feature mask", async t => {
  const f = await fixture(t);
  const result = await f.run();
  assert.equal(result.code, 0, result.output);
  assert.match(result.output, /3 record\(s\).*featureBits=0x1/);
  assert.match(result.output, /decoderFeatureBits=0x1/);
});

test("requires appearance support from actual file headers, not marker record base", async t => {
  const f = await fixture(t);
  for (const bits of [undefined, 0]) {
    f.setMarker({ ...f.marker, decoderFeatureBits: bits });
    const result = await f.run();
    assert.notEqual(result.code, 0);
    assert.match(result.output, /does not accept ACLI featureBits=0x1/);
  }
});

test("omitted/null decoderFeatureBits defaults to zero for feature-free trees", async t => {
  const f = await fixture(t); f.write(plain());
  for (const bits of [undefined, null, 0, 1, 0xffff]) {
    f.setMarker({ ...f.marker, decoderFeatureBits: bits });
    const result = await f.run();
    assert.equal(result.code, 0, result.output);
  }
});

test("rejects invalid marker feature masks even for feature-free files", async t => {
  const f = await fixture(t); f.write(plain());
  for (const bits of [-1, 0x10000, 1.5, "1", true, {}, []]) {
    f.setMarker({ ...f.marker, decoderFeatureBits: bits });
    const result = await f.run();
    assert.notEqual(result.code, 0, JSON.stringify(bits));
    assert.match(result.output, /decoderFeatureBits must be a uint16/);
  }
});

test("unions header feature bits across the complete tree", async t => {
  const f = await fixture(t); f.write(plain(0x126), "0x0126.bin");
  f.setMarker({ ...f.marker, decoderFeatureBits: 0 });
  assert.notEqual((await f.run()).code, 0);
  f.setMarker({ ...f.marker, decoderFeatureBits: 1 });
  const result = await f.run();
  assert.equal(result.code, 0, result.output);
  assert.match(result.output, /2 file\(s\), 4 record\(s\).*featureBits=0x1/);
});

test("rejects unknown header features even if marker advertises them", async t => {
  const f = await fixture(t); f.setMarker({ ...f.marker, decoderFeatureBits: 0xffff });
  for (const bits of [2, 3, 0x8000]) {
    const bytes = golden(); bytes.writeUInt16LE(bits, 6); f.write(bytes);
    const result = await f.run();
    assert.notEqual(result.code, 0); assert.match(result.output, /unknown ACLI feature bits/);
  }
});

test("validates sized tails, reserved word and exact operation counts without fallback", async t => {
  const f = await fixture(t);
  const cases = [];
  // First record's sized tail begins after 60 fixed bytes and 6 override bytes.
  for (const size of [1, 11, 12, 35, 37, 0xffffffff]) {
    const bytes = golden(); bytes.writeUInt32LE(size, 82); cases.push(bytes);
  }
  for (const at of [90, 92, 94]) {
    const bytes = golden(); bytes.writeUInt16LE(0xffff, at); cases.push(bytes);
  }
  const reserved = golden(); reserved[96] = 1; cases.push(reserved);
  cases.push(golden().subarray(0, 82)); // missing size
  cases.push(golden().subarray(0, 100)); // incomplete payload
  cases.push(Buffer.concat([golden(), Buffer.from([0])]));
  for (const bytes of cases) {
    f.write(bytes); const result = await f.run();
    assert.notEqual(result.code, 0, `accepted ${bytes.length} bytes`);
    assert.doesNotMatch(result.output, /client marker OK/);
  }
});

test("feature bit needs lengths for absent records; zero-record header still requires its features", async t => {
  const f = await fixture(t);
  const bytes = plain(); bytes[6] = 1;
  f.write(bytes); assert.notEqual((await f.run()).code, 0);
  f.write(Buffer.concat([bytes, Buffer.alloc(4)]));
  assert.equal((await f.run()).code, 0);
  const empty = bytes.subarray(0, 16); empty.writeUInt32LE(0, 12);
  f.write(empty); f.setMarker({ ...f.marker, decoderFeatureBits: 0 });
  assert.notEqual((await f.run()).code, 0);
});

test("still rejects old base layouts and mismatched block ids", async t => {
  const f = await fixture(t);
  for (const size of [52, 56]) {
    f.write(plain().subarray(0, 16 + size));
    const result = await f.run();
    assert.notEqual(result.code, 0); assert.match(result.output, /regenerate with the 60-byte generator/);
  }
  f.write(plain(0x126));
  const result = await f.run();
  assert.notEqual(result.code, 0); assert.match(result.output, /file contains landblock 0x0126/);
});
