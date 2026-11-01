import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const script = new URL("./upload_landblock_instances.sh", import.meta.url).pathname;

// All datasets, credentials and publisher commands are synthetic and temporary.
// No real R2 calls, uploads or exporter invocations are made by these tests.
async function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "acli-publish-test-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const src = path.join(root, "src");
  fs.mkdirSync(src);
  const bytes = Buffer.alloc(16 + 60);
  bytes.write("ACLI"); bytes[4] = 1; bytes[5] = 1;
  bytes.writeUInt32LE(0x125, 8); bytes.writeUInt32LE(1, 12);
  fs.writeFileSync(path.join(src, "0x0125.bin"), bytes);
  const log = path.join(root, "commands");
  const mock = path.join(root, "publisher");
  fs.writeFileSync(mock, `#!/bin/sh
printf '%s\\n' "$*" >>"$TEST_LOG"
if [ "$1" = r2 ] && [ "$3" = get ]; then
  case "$MOCK_STATE" in
    reserved) exit 0 ;;
    auth) echo 'Authentication failed' >&2; exit 1 ;;
    object) case "$4" in *_publish.json) ;; *) exit 0 ;; esac ;;
  esac
  echo 'The specified key does not exist.' >&2; exit 1
fi
if [ "$1" = lsf ] && [ "$MOCK_STATE" = reserved ]; then echo '_publish.json'; fi
exit 0
`, { mode: 0o755 });
  fs.symlinkSync(mock, path.join(root, "rclone"));
  const marker = { format: "ac-noclip/acli-layout", version: 1,
    kind: 1, decoderRecordBaseBytes: [60] };
  const server = http.createServer((_req, res) => {
    res.setHeader("content-type", "application/json");
    res.end(JSON.stringify(marker));
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  t.after(() => new Promise(resolve => server.close(resolve)));
  const env = { ...process.env, PATH: `${root}:${process.env.PATH}`,
    SRC_DIR: src, BUCKET: "test-bucket", PUBLISHER: "wrangler", WRANGLER: mock,
    REVISION: "spawns-v1", TEST_LOG: log, MOCK_STATE: "missing", PARALLEL: "1",
    AWS_ACCESS_KEY_ID: "fake", AWS_SECRET_ACCESS_KEY: "fake",
    AWS_ENDPOINT_URL: "https://example.invalid",
    CLIENT_LAYOUT_URL: `http://127.0.0.1:${server.address().port}/acli-layout.json` };
  delete env.DRYRUN;
  delete env.CONFIRM_LEGACY_FULL_TREE;
  return {
    src,
    marker,
    commands: () => fs.existsSync(log) ? fs.readFileSync(log, "utf8") : "",
    run: (overrides = {}) => new Promise(resolve => {
      const vars = { ...env, ...overrides };
      for (const key of Object.keys(vars)) if (vars[key] === undefined) delete vars[key];
      const child = spawn("sh", [script], { env: vars });
      let output = "";
      child.stdout.on("data", data => output += data);
      child.stderr.on("data", data => output += data);
      child.on("close", code => resolve({ code, output }));
    }),
  };
}

test("revision validation rejects invalid, empty and oversized names before publisher calls", async t => {
  const f = await fixture(t);
  for (const REVISION of ["", "../old", "a/b", "a.b", "é", "a b", "a".repeat(65)]) {
    assert.notEqual((await f.run({ REVISION })).code, 0, REVISION);
  }
  assert.equal(f.commands(), "");
});

test("Wrangler dryrun prints exact staging prefix and performs no writes", async t => {
  const f = await fixture(t);
  const result = await f.run({ DRYRUN: "1" });
  assert.equal(result.code, 0, result.output);
  assert.match(result.output, /s3:\/\/test-bucket\/world\/landblock-instances-revisions\/spawns-v1\//);
  assert.match(result.output, /dry run\) put world\/landblock-instances-revisions\/spawns-v1\/0x0125.bin/);
  assert.doesNotMatch(f.commands(), /r2 object put/);
});

test("Wrangler reserves revision, probes object, and puts only into revision prefix", async t => {
  const f = await fixture(t);
  const result = await f.run();
  assert.equal(result.code, 0, result.output);
  const commands = f.commands();
  assert.match(commands, /put test-bucket\/world\/landblock-instances-revisions\/spawns-v1\/_publish.json/);
  assert.match(commands, /get test-bucket\/world\/landblock-instances-revisions\/spawns-v1\/0x0125.bin/);
  assert.match(commands, /put test-bucket\/world\/landblock-instances-revisions\/spawns-v1\/0x0125.bin/);
  assert.doesNotMatch(commands, /test-bucket\/world\/landblock-instances\//);
});

for (const state of ["reserved", "auth", "object"]) {
  test(`Wrangler fails closed on ${state}, never overwrites dataset objects`, async t => {
    const f = await fixture(t);
    const result = await f.run({ MOCK_STATE: state });
    assert.notEqual(result.code, 0, result.output);
    assert.doesNotMatch(f.commands(), /put .*0x0125.bin/);
  });
}

test("rclone revision uses copy --immutable, not sync, and dryrun never reserves", async t => {
  const f = await fixture(t);
  const result = await f.run({ PUBLISHER: "rclone", DRYRUN: "1", REVISION: "A_z-09" });
  assert.equal(result.code, 0, result.output);
  assert.match(f.commands(), /copy .*:s3:test-bucket\/world\/landblock-instances-revisions\/A_z-09/);
  assert.match(f.commands(), /--dry-run --progress --immutable/);
  assert.doesNotMatch(f.commands(), /copyto|sync /);
});

test("rclone refuses a nonempty revision prefix", async t => {
  const f = await fixture(t);
  const result = await f.run({ PUBLISHER: "rclone", MOCK_STATE: "reserved" });
  assert.notEqual(result.code, 0, result.output);
  assert.doesNotMatch(f.commands(), /copyto|copy |sync /);
});

test("legacy requires explicit full-tree confirmation and retains sync destination", async t => {
  const f = await fixture(t);
  assert.notEqual((await f.run({ REVISION: undefined })).code, 0);
  assert.equal(f.commands(), "");
  const result = await f.run({ REVISION: undefined, CONFIRM_LEGACY_FULL_TREE: "1",
    PUBLISHER: "rclone", DRYRUN: "1" });
  assert.equal(result.code, 0, result.output);
  assert.match(f.commands(), /sync .*:s3:test-bucket\/world\/landblock-instances /);
  assert.doesNotMatch(f.commands(), /revisions/);
});

test("attachments require deployed mask3 before any rclone/Wrangler call or reservation", async t => {
  const f = await fixture(t);
  const bytes = Buffer.from(fs.readFileSync(new URL("../tests/fixtures/acli-attachments.hex", import.meta.url), "utf8").trim(), "hex");
  fs.rmSync(path.join(f.src, "0x0125.bin"));
  fs.writeFileSync(path.join(f.src, "0x1234.bin"), bytes);
  f.marker.decoderFeatureBits = 1;
  for (const PUBLISHER of ["rclone", "wrangler"]) {
    for (const DRYRUN of ["0", "1"]) {
      const rejected = await f.run({ PUBLISHER, DRYRUN });
      assert.notEqual(rejected.code, 0, rejected.output);
      assert.match(rejected.output, /does not accept ACLI featureBits=0x3/);
      assert.equal(f.commands(), "");
    }
  }
  // Even a compatible marker cannot permit invalid recursive data.
  f.marker.decoderFeatureBits = 3;
  const invalid = Buffer.from(bytes); invalid.writeFloatLE(Infinity, 240);
  fs.writeFileSync(path.join(f.src, "0x1234.bin"), invalid);
  const rejected = await f.run({ PUBLISHER: "rclone" });
  assert.notEqual(rejected.code, 0, rejected.output);
  assert.equal(f.commands(), "");
  fs.writeFileSync(path.join(f.src, "0x1234.bin"), bytes);
  const accepted = await f.run({ PUBLISHER: "rclone", DRYRUN: "1" });
  assert.equal(accepted.code, 0, accepted.output);
  assert.match(f.commands(), /copy .*--immutable/);
  assert.doesNotMatch(f.commands(), /copyto|sync /);
});

test("appearance support is checked before any publisher call, including reservation", async t => {
  const f = await fixture(t);
  const bytes = Buffer.from(fs.readFileSync(new URL("../tests/fixtures/acli-appearance.hex", import.meta.url), "utf8").trim(), "hex");
  fs.writeFileSync(path.join(f.src, "0x0125.bin"), bytes);
  const rejected = await f.run({ DRYRUN: "1" });
  assert.notEqual(rejected.code, 0, rejected.output);
  assert.match(rejected.output, /does not accept ACLI featureBits=0x1/);
  assert.equal(f.commands(), "");
  f.marker.decoderFeatureBits = 1;
  const accepted = await f.run({ DRYRUN: "1" });
  assert.equal(accepted.code, 0, accepted.output);
  assert.match(accepted.output, /featureBits=0x1/);
  assert.doesNotMatch(f.commands(), /r2 object put/);
});

test("empty, malformed, wrong-layout and non-flat trees cannot reach publisher", async t => {
  const f = await fixture(t);
  const file = path.join(f.src, "0x0125.bin");
  fs.unlinkSync(file);
  assert.notEqual((await f.run()).code, 0);
  fs.writeFileSync(file, "not ACLI");
  assert.notEqual((await f.run()).code, 0);
  const legacy = Buffer.alloc(16 + 52);
  legacy.write("ACLI"); legacy[4] = 1; legacy[5] = 1;
  legacy.writeUInt32LE(0x125, 8); legacy.writeUInt32LE(1, 12);
  fs.writeFileSync(file, legacy);
  assert.notEqual((await f.run()).code, 0);
  fs.mkdirSync(path.join(f.src, "nested"));
  assert.notEqual((await f.run()).code, 0);
  assert.equal(f.commands(), "");
});
