import assert from 'node:assert/strict';
import { gzipSync } from 'node:zlib';
import test from 'node:test';
import worker from './area_pack_upload_worker.mjs';

const secret = 'a'.repeat(64);
const filename = `area-${'b'.repeat(64)}.bin`;
const key = `world/area-packs/${filename}`;
const bytes = new Uint8Array([0, 1, 2, 0xff, 0x1f, 0x8b]);

// Node stand-in for the Workers-native FixedLengthStream.
globalThis.FixedLengthStream = class {
  constructor(length) {
    let count = 0;
    const stream = new TransformStream({
      transform(chunk, controller) {
        count += chunk.byteLength;
        if (count > length) throw new TypeError('FixedLengthStream overflow');
        controller.enqueue(chunk);
      },
      flush() {
        if (count !== length) throw new TypeError('FixedLengthStream underflow');
      },
    });
    this.readable = stream.readable;
    this.writable = stream.writable;
  }
};

class FakeR2 {
  objects = new Map();
  calls = [];
  fail = false;
  earlyConflict = false;
  async list(options) {
    this.calls.push(['list', options]);
    if (this.fail) throw new Error('private error');
    const names = [...this.objects.keys()].filter(k => k.startsWith(options.prefix)).sort();
    const offset = options.cursor ? Number(options.cursor) : 0;
    const page = names.slice(offset, offset + options.limit);
    return { objects: page.map(key => ({ key, size: this.objects.get(key).byteLength, etag: `etag-${key}` })),
      truncated: offset + page.length < names.length,
      cursor: String(offset + page.length) };
  }
  async get(key) {
    this.calls.push(['get', key]);
    if (this.fail) throw new Error('private error');
    const data = this.objects.get(key);
    return data ? { body: new Blob([data]).stream(), size: data.length } : null;
  }
  async put(key, body, options) {
    this.calls.push(['put', key, options]);
    if (this.fail) throw new Error('private error');
    assert.equal(options.onlyIf.get('if-none-match'), '*');
    assert.deepEqual(options.httpMetadata, {
      contentType: 'application/octet-stream', cacheControl: 'public, max-age=31536000, immutable',
    });
    if (this.earlyConflict && this.objects.has(key)) return null;
    const data = new Uint8Array(await new Response(body).arrayBuffer());
    if (this.objects.has(key)) return null; // conditional put, including concurrent writes
    this.objects.set(key, data);
    return { key };
  }
}

const env = () => ({ UPLOAD_KEY: secret, EXTRACTED_BUCKET: new FakeR2() });
const send = (e, path = '/objects', init = {}) => worker.fetch(
  new Request(`https://publisher.invalid${path}`, {
    ...init, headers: { Authorization: `Bearer ${secret}`, ...init.headers },
    ...(init.body ? { duplex: 'half' } : {}),
  }), e);
const put = (e, data = bytes, path = `/objects/${filename}`, overrides = {}) => send(e, path, {
  method: 'PUT', body: data, headers: { 'Content-Length': String(data.length),
    'X-Area-Pack-Bytes': String(bytes.length), 'X-Area-Pack-Transfer': 'identity',
    ...overrides },
});
const status = async (response, expected) => {
  assert.equal(response.status, expected);
  assert.equal(response.headers.get('cache-control'), 'no-store');
};

test('authentication rejects absent, wrong, invalid-configured secret before any R2 operations', async () => {
  const e = env();
  for (const auth of [undefined, 'Bearer wrong', `Bearer ${'c'.repeat(64)}`, `Basic ${secret}`]) {
    await status(await send(e, '/objects', { headers: { Authorization: auth } }), 401);
    await status(await put(e, bytes, `/objects/${filename}`, { Authorization: auth }), 401);
  }
  e.UPLOAD_KEY = undefined;
  await status(await send(e), 401);
  assert.equal(e.EXTRACTED_BUCKET.calls.length, 0);
});

test('routes constrain namespace, filename, query, and methods', async () => {
  const e = env();
  for (const path of ['/objects/other.bin', `/objects/${filename.toUpperCase()}`,
    '/objects/../private', '/objects/%2e%2e%2fprivate', `/objects/${filename}/extra`, '/private']) {
    await status(await send(e, path), 404);
  }
  await status(await send(e, '/objects?prefix=world/'), 400);
  await status(await send(e, `/objects/${filename}?cursor=x`), 400);
  await status(await send(e, '/objects', { method: 'DELETE' }), 405);
  await status(await send(e, `/objects/${filename}`, { method: 'DELETE' }), 405);
  assert.equal(e.EXTRACTED_BUCKET.calls.length, 0);
});

test('listing uses only fixed prefix and 1000-item cursor pages', async () => {
  const e = env();
  e.EXTRACTED_BUCKET.objects.set('private/data', bytes);
  for (let i = 0; i < 1001; i++) e.EXTRACTED_BUCKET.objects.set(`world/area-packs/area-${i.toString(16).padStart(64, '0')}.bin`, bytes);
  const first = await send(e);
  await status(first, 200);
  const page = await first.json();
  assert.equal(page.objects.length, 1000);
  assert.equal(page.truncated, true);
  assert.equal(page.cursor, '1000');
  assert.deepEqual(Object.keys(page.objects[0]), ['key', 'size', 'etag']);
  const next = await send(e, `/objects?cursor=${page.cursor}`);
  await status(next, 200);
  assert.equal((await next.json()).objects.length, 1);
  assert.deepEqual(e.EXTRACTED_BUCKET.calls, [
    ['list', { prefix: 'world/area-packs/', limit: 1000 }],
    ['list', { prefix: 'world/area-packs/', limit: 1000, cursor: '1000' }],
  ]);
});

test('identity and gzip transfers write opaque raw and .br objects, GET never decodes .br', async () => {
  const e = env();
  await status(await put(e), 201);
  const compressed = gzipSync(bytes);
  const brPath = `/objects/${filename}.br`;
  await status(await put(e, compressed, brPath, {
    'Content-Length': String(compressed.length), 'X-Area-Pack-Transfer': 'gzip',
  }), 201);
  for (const [path, expected] of [[`/objects/${filename}`, bytes], [brPath, bytes]]) {
    const res = await send(e, path);
    await status(res, 200);
    assert.equal(res.headers.get('content-type'), 'application/octet-stream');
    assert.equal(res.headers.get('content-length'), String(expected.length));
    assert.equal(res.headers.get('content-encoding'), null);
    assert.deepEqual(new Uint8Array(await res.arrayBuffer()), expected);
  }
  await status(await send(e, `/objects/${'a'.repeat(64)}.bin`), 404);
});

test('conditional writes cannot overwrite even if a concurrent writer wins after upload starts', async () => {
  const e = env();
  await status(await put(e), 201);
  await status(await put(e), 412);
  e.EXTRACTED_BUCKET.earlyConflict = true;
  await status(await put(e), 412); // R2 declines before draining the readable
  assert.deepEqual(e.EXTRACTED_BUCKET.objects.get(key), bytes);
  e.EXTRACTED_BUCKET.earlyConflict = false;
  const other = `/objects/area-${'c'.repeat(64)}.bin`;
  const original = e.EXTRACTED_BUCKET.put.bind(e.EXTRACTED_BUCKET);
  e.EXTRACTED_BUCKET.put = async (...args) => {
    e.EXTRACTED_BUCKET.objects.set('world/area-packs/' + other.slice('/objects/'.length), new Uint8Array([9]));
    return original(...args);
  };
  await status(await put(e, bytes, other), 412);
  assert.deepEqual(e.EXTRACTED_BUCKET.objects.get(`world/area-packs/${other.slice('/objects/'.length)}`), new Uint8Array([9]));
});

test('invalid lengths and encoding never persist objects', async () => {
  for (const overrides of [
    { 'Content-Length': '7' }, { 'Content-Length': '5', 'X-Area-Pack-Bytes': '5' },
    { 'Content-Length': '' }, { 'X-Area-Pack-Bytes': '0' },
    { 'X-Area-Pack-Bytes': String(256 * 1024 * 1024 + 1) },
    { 'X-Area-Pack-Bytes': '7' }, { 'X-Area-Pack-Transfer': 'br' },
    { 'Content-Encoding': 'gzip' },
  ]) {
    const e = env();
    await status(await put(e, bytes, `/objects/${filename}`, overrides), 400);
    assert.equal(e.EXTRACTED_BUCKET.objects.size, 0);
  }
  for (const data of [new Uint8Array([1, 2, 3]), gzipSync(bytes).subarray(0, 7)]) {
    const e = env();
    await status(await put(e, data, `/objects/${filename}`, {
      'X-Area-Pack-Transfer': 'gzip', 'X-Area-Pack-Bytes': String(bytes.length),
    }), 400);
    assert.equal(e.EXTRACTED_BUCKET.objects.size, 0);
  }
  const compressed = gzipSync(bytes);
  for (const headers of [
    { 'X-Area-Pack-Bytes': String(bytes.length - 1) },
    { 'X-Area-Pack-Bytes': String(bytes.length + 1) },
    { 'Content-Length': String(compressed.length - 1) },
    { 'Content-Length': String(compressed.length + 1) },
  ]) {
    const e = env();
    await status(await put(e, compressed, `/objects/${filename}`, {
      'X-Area-Pack-Transfer': 'gzip', ...headers,
    }), 400);
    assert.equal(e.EXTRACTED_BUCKET.objects.size, 0);
  }
});

test('R2 errors are 500, not 404; missing binding is 500', async () => {
  const e = env(); e.EXTRACTED_BUCKET.fail = true;
  await status(await send(e), 500);
  await status(await send(e, `/objects/${filename}`), 500);
  await status(await put(e), 500);
  await status(await send({ UPLOAD_KEY: secret }), 500);
});
