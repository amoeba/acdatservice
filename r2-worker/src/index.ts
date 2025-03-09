import { R2Bucket } from '@cloudflare/workers-types';
import { PhotonImage } from "@cf-wasm/photon";

interface Env {
  DATS_BUCKET: R2Bucket;
}

const Objects = {
  "PORTAL_DAT": { "key": "portal.dat", "contentType": "application/octet-stream" },
  "CELL_DAT": { "key": "cell.dat", "contentType": "application/octet-stream" }
}

const handle = async function (request, env, ctx): Promise<Response> {
  let url = new URL(request.url);
  // TODO: Appropraite error response ofr invalid request

  let file_type = url.searchParams.get("type");
  let id = url.searchParams.get("id");

  // TODO: Error response for invalid input
  const obj = Objects.PORTAL_DAT;

  const startByte = 0;
  const numberOfBytes = 5;

  const object = await env.DATS_BUCKET.get(obj.key, {
    range: {
      offset: startByte,
      length: numberOfBytes
    }
  });

  // let buf = new Uint8Array(4096);

  // for (let i = 0; i < buf.length; i++) {
  // buf[i] = 0;
  // }

  const imageUrl = "https://avatars.githubusercontent.com/u/314135";
  const inputBytes = await fetch(imageUrl)
    .then((res) => res.arrayBuffer())
    .then((buffer) => new Uint8Array(buffer));

  const inputImage = PhotonImage.new_from_byteslice(inputBytes);
  let out = inputImage.get_bytes_webp();
  inputImage.free();


  const headers = new Headers();
  headers.set("Content-Type", "image/webp");


  return new Response(out, { status: 200, headers });
}

export default {
  async fetch(request, env, ctx): Promise<Response> {
    return await handle(request, env, ctx);

    // const obj = Objects.PORTAL_DAT;

    // const startByte = 0;
    // const numberOfBytes = 5;

    // const object = await env.DATS_BUCKET.get(obj.key, {
    //   range: {
    //     offset: startByte,
    //     length: numberOfBytes
    //   }
    // });

    // if (object === null) {
    //   return new Response("Object Not Found", { status: 404 });
    // }

    // const headers = new Headers();
    // headers.set("Content-Type", obj.contentType);
    // headers.set("Content-Range", `bytes ${startByte}-${startByte + numberOfBytes - 1}/${object.size}`);

    // return new Response(object.body, { status: 206, headers });
  },
} satisfies ExportedHandler<Env>;
