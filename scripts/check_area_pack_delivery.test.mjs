import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { brotliCompressSync } from 'node:zlib';
import { checkAreaPackDelivery, checkCatalogDelivery, sampleCatalog, validateCatalog } from './check_area_pack_delivery.mjs';

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
  const d = () => ({ file: `area-${hash}.bin`, bytes, areaId: 'dungeon-01230100', blocks: [0x0123], acliRevision: 'appearance-v3', cellRanges: [[0x01230100, 0x01230146]] });
  assert.throws(() => sampleCatalog([], 0), /positive integer/);
  await assert.rejects(checkCatalogDelivery({ bad: { file: 'wrong', bytes: 1, areaId: 1, blocks: 1, acliRevision: 1 } }, { origin: prefix }), /Invalid filename/);
  const result = await checkCatalogDelivery({ a: d(), b: d() }, { origin: prefix, sample: 4 });
  assert.equal(result.selected, 1);
  assert.equal(result.results[0].checks.length, 3);
});

test('catalog metadata uses actual ACAP string identities and block arrays, not numeric placeholders', () => {
  const base = { file: `area-${'a'.repeat(64)}.bin`, bytes: 2683517, areaId: 'dungeon-01250100', blocks: [0x0125], acliRevision: 'appearance-v3', cellRanges: [[0x01250100, 0x01250146]] };
  assert.deepEqual(validateCatalog({ '01250100': base }), [base]);
  const outdoor = { ...base, areaId: 'area-f518-r2', blocks: [0xf417, 0xf418, 0xf518], cellRanges: undefined };
  assert.deepEqual(validateCatalog({ 'Ithaenc Cathedral': outdoor }), [outdoor]);
  for (const [field, value] of [['areaId', 1], ['blocks', 1], ['blocks', [0x10000]], ['blocks', [1, 1]], ['acliRevision', 1], ['cellRanges', [[-1, 1]]]]) {
    assert.throws(() => validateCatalog({ bad: { ...base, [field]: value } }), /Invalid/);
  }
});

test('catalog bulk checks reject HTTP failures and double encoding', async t => {
  for (const options of [{ fail: true }, { doubleEncoded: true }]) {
    const { url, hash, bytes } = await fixture(t, options);
    const origin = url.slice(0, url.lastIndexOf('/') + 1);
    const d = { file: `area-${hash}.bin`, bytes, areaId: 'area-f518-r2', blocks: [0xf518], acliRevision: 'appearance-v3' };
    await assert.rejects(checkCatalogDelivery({ x: d }, { origin }), options.fail ? /503/ : /Decoded body hash/);
  }
});
