import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { brotliCompressSync } from 'node:zlib';
import { checkAreaPackDelivery } from './check_area_pack_delivery.mjs';

for (const doubleEncoded of [false, true]) {
  test(doubleEncoded ? 'rejects the edge double-Brotli regression' : 'accepts a single Brotli layer and raw/q=0 representations', async t => {
    const raw = Buffer.from('ACAP test bytes');
    const hash = createHash('sha256').update(raw).digest('hex');
    const server = createServer((req, res) => {
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
    const url = `http://127.0.0.1:${server.address().port}/area-${hash}.bin`;
    if (doubleEncoded) await assert.rejects(checkAreaPackDelivery(url), /Decoded body hash/);
    else assert.equal((await checkAreaPackDelivery(url)).length, 3);
  });
}
