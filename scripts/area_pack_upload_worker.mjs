// Temporary, private area-pack publisher. Deploy separately from the public service.
const PREFIX = 'world/area-packs/';
const NAME = /^area-[0-9a-f]{64}\.bin(?:\.br)?$/;
const MAX_BYTES = 256 * 1024 * 1024;
const MAX_WIRE_BYTES = 512 * 1024 * 1024;
const CACHE_CONTROL = 'public, max-age=31536000, immutable';

function reply(status, body = null, headers = {}) {
  return new Response(body, { status, headers: { 'Cache-Control': 'no-store', ...headers } });
}

// Compare exactly 64 ASCII code units even when the supplied token is malformed.
// Never use an early-return string equality check for the bearer secret.
function authorized(request, secret) {
  const header = request.headers.get('Authorization') ?? '';
  const token = header.startsWith('Bearer ') ? header.slice(7) : '';
  let difference = token.length ^ 64;
  for (let i = 0; i < 64; i++) {
    difference |= (token.charCodeAt(i) || 0) ^ (secret?.charCodeAt(i) || 0);
  }
  return typeof secret === 'string' && /^[0-9a-fA-F]{64}$/.test(secret) && difference === 0;
}

function decimal(value) {
  if (value === null || !/^(0|[1-9][0-9]*)$/.test(value)) return null;
  const number = Number(value);
  return Number.isSafeInteger(number) ? number : null;
}

function countBytes(expected, invalid) {
  let count = 0;
  return new TransformStream({
    transform(chunk, controller) {
      if (!(chunk instanceof Uint8Array) || count + chunk.byteLength > expected) {
        invalid();
        throw new Error('invalid byte length');
      }
      count += chunk.byteLength;
      controller.enqueue(chunk);
    },
    flush() {
      if (count !== expected) {
        invalid();
        throw new Error('length mismatch');
      }
    },
  });
}

async function upload(request, bucket, key) {
  const wireBytes = decimal(request.headers.get('Content-Length'));
  const decodedBytes = decimal(request.headers.get('X-Area-Pack-Bytes'));
  const transfer = request.headers.get('X-Area-Pack-Transfer');
  // HTTP Content-Encoding is deliberately not used for the transfer encoding:
  // the body must reach this Worker without transparent HTTP decompression.
  if (!request.body || wireBytes === null || wireBytes < 1 || wireBytes > MAX_WIRE_BYTES ||
      decodedBytes === null || decodedBytes < 1 || decodedBytes > MAX_BYTES ||
      !['identity', 'gzip'].includes(transfer) || request.headers.has('Content-Encoding') ||
      (transfer === 'identity' && decodedBytes !== wireBytes)) return reply(400);

  let invalidInput = false;
  // Identity is the bulk path on Workers Free: the declared wire and decoded
  // lengths already agree, and native FixedLengthStream validates actual bytes.
  // Avoid per-chunk JS transforms (and their CPU cost) for these large objects.
  let source = request.body;
  if (transfer === 'gzip') {
    const markInvalid = () => { invalidInput = true; };
    source = source.pipeThrough(countBytes(wireBytes, markInvalid));
    source = source.pipeThrough(new DecompressionStream('gzip'));
    source = source.pipeThrough(countBytes(decodedBytes, markInvalid));
  }

  // FixedLengthStream provides an exact-size readable to R2 without buffering a
  // whole pack. The pipe and the put must run concurrently: awaiting either one
  // first can deadlock on backpressure or an early conditional-write rejection.
  const fixed = new FixedLengthStream(decodedBytes);
  const stop = new AbortController();
  const pumping = source.pipeTo(fixed.writable, { signal: stop.signal });
  const putting = Promise.resolve().then(() => bucket.put(key, fixed.readable, {
    onlyIf: new Headers({ 'if-none-match': '*' }),
    httpMetadata: { contentType: 'application/octet-stream', cacheControl: CACHE_CONTROL },
  }));
  // Convert both outcomes to values immediately so an early error cannot become
  // an unhandled rejection while the other stream is being cancelled.
  const pipeResult = pumping.then(() => ({ ok: true }), error => ({ ok: false, error }));
  const putResult = Promise.resolve(putting).then(value => ({ ok: true, value }), error => ({ ok: false, error }));
  const first = await Promise.race([
    pipeResult.then(result => ({ side: 'pipe', result })),
    putResult.then(result => ({ side: 'put', result })),
  ]);
  if (first.side === 'pipe' && !first.result.ok) {
    stop.abort();
    // A failed upload is not a missing object. Do not wait for a broken R2
    // consumer to finish draining a stream that has already failed.
    return reply(400);
  }
  if (first.side === 'put' && (!first.result.ok || first.result.value === null)) {
    // R2 may decline the conditional put without ever reading the body. Cancel
    // the unconsumed readable as well as the pipe: abort alone cannot unblock
    // an in-flight write waiting for FixedLengthStream backpressure.
    stop.abort();
    void fixed.readable.cancel().catch(() => {}); // already locked if R2 read it
    const pipe = await pipeResult;
    // An upstream stream error can reach the R2 reader before pipeTo rejects.
    // A separate R2/network failure must remain 500, not a false 400 caused
    // by our own cancellation of the producer.
    return reply(first.result.ok ? 412 :
      (invalidInput || (!pipe.ok && pipe.error === first.result.error)) ? 400 : 500);
  }
  const pipe = await pipeResult;
  if (!pipe.ok) return reply(400);
  const put = await putResult;
  if (!put.ok) return reply(500);
  if (put.value === null) return reply(412);
  return reply(201);
}

export default {
  async fetch(request, env) {
    if (!authorized(request, env?.UPLOAD_KEY)) return reply(401);
    try {
      if (!env.EXTRACTED_BUCKET) return reply(500);
      const url = new URL(request.url);
      const list = url.pathname === '/objects';
      const name = url.pathname.startsWith('/objects/') ? url.pathname.slice('/objects/'.length) : '';
      if (!list && !NAME.test(name)) return reply(404);
      if (list) {
        if (request.method !== 'GET') return reply(405, null, { Allow: 'GET' });
        if ([...url.searchParams.keys()].some(key => key !== 'cursor') ||
            url.searchParams.getAll('cursor').length > 1) return reply(400);
        const cursor = url.searchParams.get('cursor');
        if (cursor !== null && (!cursor || cursor.length > 4096)) return reply(400);
        const result = await env.EXTRACTED_BUCKET.list({ prefix: PREFIX, limit: 1000,
          ...(cursor === null ? {} : { cursor }) });
        return reply(200, JSON.stringify({
          objects: result.objects.map(({ key, size, etag }) => ({ key, size, etag })),
          truncated: result.truncated,
          cursor: result.truncated ? result.cursor : null,
        }), { 'Content-Type': 'application/json' });
      }
      if (url.search) return reply(400);
      const key = PREFIX + name;
      if (request.method === 'PUT') return await upload(request, env.EXTRACTED_BUCKET, key);
      if (request.method !== 'GET') return reply(405, null, { Allow: 'GET, PUT' });
      const object = await env.EXTRACTED_BUCKET.get(key);
      if (object === null) return reply(404);
      return new Response(object.body, {
        status: 200,
        encodeBody: 'manual',
        headers: { 'Cache-Control': 'no-store', 'Content-Type': 'application/octet-stream',
          'Content-Length': String(object.size) },
      });
    } catch {
      // Never reveal R2 errors, request contents or secrets to callers/logs.
      return reply(500);
    }
  },
};
