// Real HTTP check: validates what a normal fetch/browser sees after decoding,
// not just the bytes stored in R2. Catches Workers double-compressing Brotli.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';

export async function checkAreaPackDelivery(url) {
  const match = /\/area-([a-f0-9]{64})\.bin$/.exec(new URL(url).pathname);
  assert.ok(match, 'Expected a content-addressed area-pack URL');
  const results = [];
  for (const encoding of ['br', 'identity', 'br;q=0, identity;q=1']) {
    const response = await fetch(url, { headers: { 'Accept-Encoding': encoding } });
    assert.equal(response.status, 200);
    const bytes = new Uint8Array(await response.arrayBuffer());
    assert.equal(createHash('sha256').update(bytes).digest('hex'), match[1], `Decoded body hash (${encoding}); check for double Brotli encoding`);
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
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  if (!process.argv[2]) throw new Error('Usage: node scripts/check_area_pack_delivery.mjs https://dats.treestats.net/dats/world/area-packs/area-<sha256>.bin');
  console.log(JSON.stringify(await checkAreaPackDelivery(process.argv[2]), null, 2));
}
