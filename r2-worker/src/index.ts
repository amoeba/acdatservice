import { R2Bucket } from '@cloudflare/workers-types';

interface Env {
	DATS_BUCKET: R2Bucket;
}

export default {
	async fetch(request, env, ctx): Promise<Response> {
		const startByte = 0;
		const numberOfBytes = 5;

		const object = await env.DATS_BUCKET.get("portal.dat", {
			range: {
				offset: startByte,
				length: numberOfBytes
			}
		});

		if (object === null) {
			return new Response("Object Not Found", { status: 404 });
		}

		const headers = new Headers();
		// TODO
		// headers.set("Content-Type", object.httpMetadata?.contentType);
		headers.set("Content-Range", `bytes ${startByte}-${startByte + numberOfBytes - 1}/${object.size}`);

		return new Response(object.body, { status: 206, headers });
	},
} satisfies ExportedHandler<Env>;
