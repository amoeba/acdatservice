import { R2Bucket } from '@cloudflare/workers-types';

interface Env {
  DATS_BUCKET: R2Bucket;
}

const Objects = {
  "PORTAL_DAT": { "key": "portal.dat", "contentType": "application/octet-stream" },
  "CELL_DAT": { "key": "cell.dat", "contentType": "application/octet-stream" }
}

export default {
  async fetch(request, env, ctx): Promise<Response> {
    const obj = Objects.PORTAL_DAT;

    const startByte = 0;
    const numberOfBytes = 5;

    const object = await env.DATS_BUCKET.get(obj.key, {
      range: {
        offset: startByte,
        length: numberOfBytes
      }
    });

    if (object === null) {
      return new Response("Object Not Found", { status: 404 });
    }

    const headers = new Headers();
    headers.set("Content-Type", obj.contentType);
    headers.set("Content-Range", `bytes ${startByte}-${startByte + numberOfBytes - 1}/${object.size}`);

    return new Response(object.body, { status: 206, headers });
  },
} satisfies ExportedHandler<Env>;
