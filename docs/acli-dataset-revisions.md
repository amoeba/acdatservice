# Immutable ACLI dataset revisions

Dataset identity is separate from ACLI's binary format version. Regenerating
placements (including generated spawns) under an existing year-cached URL can
leave clients rendering stale data. A random cache-busting query alone is not
sufficient: it must select different origin objects too.

## HTTP / R2 contract

- `GET /dats/world/landblock-instances/0x0125?revision=spawns-v1` reads exactly
  `world/landblock-instances-revisions/spawns-v1/0x0125.bin` in `EXTRACTED_BUCKET`.
- Omit `revision` to read the unchanged legacy key
  `world/landblock-instances/0x0125.bin`. That tree is the rollback backup.
- Revision is case-sensitive, **1–64 ASCII characters**, `[A-Za-z0-9_-]` only.
  Empty, malformed or repeated `revision` parameters return **400**. Validation
  occurs after normal URL query decoding; traversal and non-ASCII are rejected.
- A valid but unknown revision, or a missing block in a known revision, returns
  **404**. There is one R2 lookup and **no fallback to legacy or another revision**.
- Successful responses retain `public, max-age=31536000, immutable`. New content
  needs a new revision name, never replacement of an already published object.
- The handler reads the original request URL including its query. No Worker
  cache key strips it. Normal HTTP/Cloudflare cache keys distinguish the queries;
  do not install a cache rule that ignores `revision`.
- ac-noclip pins `ACLI_DATASET_REVISION = "spawns-v1"`. Local Vite and e2e fixture
  servers deliberately ignore the query and read the configured local tree.

## Parent-controlled publication (not performed by this change)

Do not generate data, run e2e, upload or deploy until coordinated. The exporter,
its helper, filtering and generation limits are unchanged. The parent must finish
and audit the complete export, reject cap hits, and verify counts/layout before
publication. Do not modify a source directory while it is being uploaded.

The production extracted bucket in `wrangler.toml` is
`treestats-acdats-extracted`. The **staging destination for this dataset** is the
new isolated prefix in that same bucket, not a separate staging bucket:

```
s3://treestats-acdats-extracted/world/landblock-instances-revisions/spawns-v1/
```

With the existing Wrangler login, no S3 token or aws CLI is needed:

```sh
cd /Users/bryce/dev/acdatservice
# Validation + destination preview; remote read probes, NO writes.
BUCKET=treestats-acdats-extracted PUBLISHER=wrangler REVISION=spawns-v1 DRYRUN=1 \
  SRC_DIR=/Users/bryce/dev/ac-noclip/export/spawns-v1/world/landblock-instances \
  ./scripts/upload_landblock_instances.sh

# Parent ONLY, after the export is complete and audited: same command without DRYRUN.
BUCKET=treestats-acdats-extracted PUBLISHER=wrangler REVISION=spawns-v1 \
  SRC_DIR=/Users/bryce/dev/ac-noclip/export/spawns-v1/world/landblock-instances \
  ./scripts/upload_landblock_instances.sh
```

The nonempty-tree, filename/header, current-layout and deployed-client marker
guards remain mandatory, including in dryrun. `CLIENT_LAYOUT_URL` can point to
an explicitly verified compatible preview marker; it is not a bypass. The marker
guard tests decoding compatibility, not whether the new revision client is live,
so a compatible existing client permits **upload before client deployment**.
If the live marker lacks the current layout, coordinate a compatible marker/client
first; do not disable the guard.

Revision publication is copy-only, never sync/delete. Before writes, the script
reserves `_publish.json`; this is a **reservation, not a completion manifest**.
It refuses an existing reservation, even after failure. rclone requires the whole
prefix to be empty and uses `copy --immutable`. Wrangler checks the reservation
then GETs each target before PUT and refuses existing objects. Unknown probe or
authentication failures fail closed. Wrangler only offers per-object operations,
so this adds one GET per file and can be slower than rclone. Dryrun checks the
reservation (or rclone prefix) and prints the destination without writing it;
Wrangler's per-file checks run on actual publication.

**Single writer required:** Wrangler has no conditional create/PUT here. Do not
run publishers concurrently, manually write these prefixes, or delete a
reservation to retry. A failed run may have uploaded part of a revision; never
point a client at that dataset. Coordinate a new revision (and client constant)
for a new attempt. A successful upload is still subject to the parent's complete
object/count/byte audit before deployment. There is no completeness inference
from filenames and no automatic publish/deploy step.

## Deployment and rollback

1. Parent finishes generation and audit, previews, then uploads the new revision.
2. Parent verifies the complete uploaded revision, then deploys service query
   support. Probe `?revision=spawns-v1`; also verify an unknown revision gives 404
   and the unversioned route still returns the old data.
3. Only then deploy the client pinned to `spawns-v1`; coordinate e2e separately.
4. Roll back the client to its prior unversioned URL (or a previous revision).
   Keep service support and both data trees. Do not repair immutable data in place.

Legacy mode is retained by **unsetting** `REVISION` and explicitly setting
`CONFIRM_LEGACY_FULL_TREE=1`. rclone then uses the historical sync/delete prefix;
Wrangler retains its historical per-object overwrite behavior. Never set that
confirmation for a partial `--block`/`--limit` export. It is an operator assertion,
not an automatic proof of completeness. Never use legacy mode for spawns-v1.

## Source-only verification

```sh
cargo test --lib
node --test scripts/upload_landblock_instances.test.mjs
sh -n scripts/upload_landblock_instances.sh
# In ac-noclip (no Playwright/e2e):
node --import tsx --test src/ac/LandblockInstanceContentSource.test.ts
```

Uploader tests use temporary synthetic bytes, a local marker server and mocked
publishers; they cannot upload data.
