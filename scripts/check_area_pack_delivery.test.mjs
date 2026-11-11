import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { brotliCompressSync } from 'node:zlib';
import { checkAreaPackDelivery, checkCatalogDelivery, sampleCatalog } from './check_area_pack_delivery.mjs';

async function fixture(t, { doubleEncoded = false, fail = false } = {}) {
  const raw = Buffer.from('ACAP test bytes');
  const hash = createHash('sha256').update(raw).digest('hex');
  const server = createServer((req, res) => {
    if (fail) { res.writeHead(503).end(); return; }
    let body = raw;
    if (req.headers['accept-encoding'] === 'br') {
      body = brotliCompressSync(raw);
      if (doubleEncoded) body = brotliCompressSync(body);
      res.setHeader('Content-Encoding', 'br');
    }
    res.setHeader('Cache-Control', 'public, max-age=31536000, immutable');
    res.setHeader('Vary', 'Accept-Encoding');
    res.setHeader('Content-Type', 'application/octet-stream');
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.end(body);
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise(resolve => { server.close(resolve); server.closeAllConnections(); }));
  return { url: `http://127.0.0.1:${server.address().port}/area-${hash}.bin`, hash, bytes: raw.length };
}

for (const doubleEncoded of [false, true]) {
  test(doubleEncoded ? 'rejects the edge double-Brotli regression' : 'accepts br, identity and q=0 representations', async t => {
    const { url } = await fixture(t, { doubleEncoded });
    if (doubleEncoded) await assert.rejects(checkAreaPackDelivery(url), /Decoded body hash/);
    else assert.equal((await checkAreaPackDelivery(url)).length, 3);
  });
}

test('validates catalog, deduplicates filenames and samples smallest/largest', async t => {
  const { url, hash, bytes } = await fixture(t);
  const prefix = url.slice(0, url.lastIndexOf('/') + 1);
  const d = n => ({ file: `area-${hash}.bin`, bytes, areaId: n, blocks: 1, acliRevision: 1 });
  assert.throws(() => sampleCatalog([], 0), /positive integer/);
  await assert.rejects(checkCatalogDelivery({ bad: { file: 'wrong', bytes: 1, areaId: 1, blocks: 1, acliRevision: 1 } }, { origin: prefix }), /Invalid filename/);
  const result = await checkCatalogDelivery({ a: d(1), b: d(2) }, { origin: prefix, sample: 4 });
  assert.equal(result.selected, 1);
  assert.equal(result.results[0].checks.length, 3);
});

test('catalog bulk checks reject HTTP failures and double encoding', async t => {
  for (const options of [{ fail: true }, { doubleEncoded: true }]) {
    const { url, hash, bytes } = await fixture(t, options);
    const origin = url.slice(0, url.lastIndexOf('/') + 1);
    const d = { file: `area-${hash}.bin`, bytes, areaId: 0, blocks: 1, acliRevision: 1 };
    await assert.rejects(checkCatalogDelivery({ x: d }, { origin }), options.fail ? /503/ : /Decoded body hash/);
  }
});
