// Real HTTP check: validates what a normal fetch/browser sees after decoding,
// not just the bytes stored in R2. Catches Workers double-compressing Brotli.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';

const DEFAULT_ORIGIN = 'https://dats.treestats.net/dats/world/area-packs/';
const encodings = ['br', 'identity', 'br;q=0, identity;q=1'];

export async function checkAreaPackDelivery(url, expectedBytes) {
  const match = /\/area-([a-f0-9]{64})\.bin$/.exec(new URL(url).pathname);
  assert.ok(match, 'Expected a content-addressed area-pack URL');
  const results = [];
  for (const encoding of encodings) {
    const response = await fetch(url, { headers: { 'Accept-Encoding': encoding } });
    assert.equal(response.status, 200, `${url} (${encoding})`);
    const bytes = new Uint8Array(await response.arrayBuffer());
    assert.equal(createHash('sha256').update(bytes).digest('hex'), match[1], `Decoded body hash (${encoding}); check for double Brotli encoding`);
    if (expectedBytes !== undefined) assert.equal(bytes.length, expectedBytes, `Decoded length (${encoding})`);
    assert.match(response.headers.get('cache-control') ?? '', /max-age=31536000.*immutable/);
    assert.match(response.headers.get('vary') ?? '', /Accept-Encoding/i);
    assert.equal(response.headers.get('content-type'), 'application/octet-stream');
    assert.equal(response.headers.get('access-control-allow-origin'), '*');
    if (encoding === 'br') assert.equal(response.headers.get('content-encoding'), 'br');
    else assert.notEqual(response.headers.get('content-encoding'), 'br');
    results.push({ encoding, decodedBytes: bytes.length, contentLength: response.headers.get('content-length') });
  }
  return results;
}

export function validateCatalog(catalog) {
  assert.ok(catalog && typeof catalog === 'object' && !Array.isArray(catalog), 'Catalog must be a JSON object');
  const entries = [];
  for (const [key, d] of Object.entries(catalog)) {
    assert.ok(d && typeof d === 'object' && !Array.isArray(d), `Invalid descriptor: ${key}`);
    assert.match(d.file ?? '', /^area-[a-f0-9]{64}\.bin$/, `Invalid filename: ${key}`);
    assert.ok(Number.isSafeInteger(d.bytes) && d.bytes >= 0, `Invalid bytes: ${key}`);
    assert.ok(typeof d.areaId === 'string' && /^[a-z0-9][a-z0-9_-]{0,63}$/.test(d.areaId), `Invalid areaId: ${key}`);
    assert.ok(Array.isArray(d.blocks) && d.blocks.every(n => Number.isInteger(n) && n >= 0 && n <= 0xffff) && new Set(d.blocks).size === d.blocks.length, `Invalid blocks: ${key}`);
    assert.ok(typeof d.acliRevision === 'string' && /^[A-Za-z0-9_-]{1,64}$/.test(d.acliRevision), `Invalid acliRevision: ${key}`);
    if (d.cellRanges !== undefined) {
      assert.ok(Array.isArray(d.cellRanges), `Invalid cellRanges: ${key}`);
      for (const r of d.cellRanges) assert.ok(Array.isArray(r) && r.length === 2 && r.every(n => Number.isInteger(n) && n >= 0 && n <= 0xffffffff) && r[0] <= r[1], `Invalid cell range: ${key}`);
    }
    entries.push(d);
  }
  return [...new Map(entries.map(d => [d.file, d])).values()];
}

export function sampleCatalog(entries, count) {
  assert.ok(Number.isInteger(count) && count > 0, '--sample must be a positive integer');
  const sorted = [...entries].sort((a, b) => a.file.localeCompare(b.file));
  if (sorted.length <= count) return sorted;
  const smallest = sorted.reduce((a, b) => a.bytes <= b.bytes ? a : b);
  const largest = sorted.reduce((a, b) => a.bytes >= b.bytes ? a : b);
  const picked = new Set([smallest.file]);
  if (count > 1) picked.add(largest.file);
  for (let i = 0; picked.size < count && i < count * 3; i++) picked.add(sorted[Math.round(i * (sorted.length - 1) / Math.max(1, count - 1))].file);
  for (const d of sorted) { if (picked.size >= count) break; picked.add(d.file); }
  return [...picked].map(file => sorted.find(d => d.file === file));
}

export async function checkCatalogDelivery(catalog, { origin = DEFAULT_ORIGIN, sample = 10, concurrency = 4 } = {}) {
  const selected = sampleCatalog(validateCatalog(catalog), sample);
  assert.ok(Number.isInteger(concurrency) && concurrency > 0 && concurrency <= 4, 'Concurrency must be between 1 and 4');
  const results = new Array(selected.length);
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(concurrency, selected.length) }, async () => {
    while (next < selected.length) {
      const i = next++;
      const d = selected[i], url = new URL(d.file, origin).href;
      const checks = i === 0 ? await checkAreaPackDelivery(url, d.bytes) : await checkBrotliDelivery(url, d.bytes);
      results[i] = { file: d.file, bytes: d.bytes, checks };
    }
  }));
  return { selected: results.length, results };
}

async function checkBrotliDelivery(url, expectedBytes) {
  const match = /area-([a-f0-9]{64})\.bin$/.exec(new URL(url).pathname);
  const response = await fetch(url, { headers: { 'Accept-Encoding': 'br' } });
  assert.equal(response.status, 200, url);
  assert.equal(response.headers.get('content-encoding'), 'br');
  const bytes = new Uint8Array(await response.arrayBuffer());
  assert.equal(bytes.length, expectedBytes, `Decoded length: ${url}`);
  assert.equal(createHash('sha256').update(bytes).digest('hex'), match[1], `Decoded body hash; check double Brotli: ${url}`);
  assert.match(response.headers.get('cache-control') ?? '', /max-age=31536000.*immutable/);
  assert.match(response.headers.get('vary') ?? '', /Accept-Encoding/i);
  assert.equal(response.headers.get('content-type'), 'application/octet-stream');
  assert.equal(response.headers.get('access-control-allow-origin'), '*');
  return { encoding: 'br', decodedBytes: bytes.length };
}

async function main(args) {
  let url, catalogPath, sample = 10;
  for (const arg of args) {
    if (arg.startsWith('--catalog=')) catalogPath = arg.slice(10);
    else if (arg.startsWith('--sample=')) sample = Number(arg.slice(9));
    else if (!url) url = arg;
    else throw new Error(`Unknown argument: ${arg}`);
  }
  if (catalogPath) {
    const fs = await import('node:fs/promises');
    console.log(JSON.stringify(await checkCatalogDelivery(JSON.parse(await fs.readFile(catalogPath, 'utf8')), { sample }), null, 2));
  } else {
    if (!url) throw new Error('Usage: node scripts/check_area_pack_delivery.mjs <URL> OR --catalog=/path/runtime.json [--sample=N]');
    console.log(JSON.stringify(await checkAreaPackDelivery(url), null, 2));
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) await main(process.argv.slice(2));
