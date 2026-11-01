# acdatservice

Service for dynamically serving Asheron's Call Cell and Portal DAT resources.
Built with Cloudflare Workers, R2, and D1.

## API Routes

| Route | Description | Example |
|-------|-------------|---------|
| [`/`](https://dats.treestats.net/) | OpenAPI specification | [`https://dats.treestats.net/`](https://dats.treestats.net/) |
| [`/dats`](https://dats.treestats.net/dats) | List available DATs with file counts, sizes, and sha256 hashes | [`https://dats.treestats.net/dats`](https://dats.treestats.net/dats) |
| [`/dats/:dat/files`](https://dats.treestats.net/dats/portal/files) | List file metadata as JSON Lines (paginated, default and maximum limit 1000). DAT names: `portal`, `cell`, `highres`, `local_english` | [`https://dats.treestats.net/dats/portal/files?limit=100&offset=0`](https://dats.treestats.net/dats/portal/files?limit=100&offset=0) |
| [`/dats/:dat/files/:file_id`](https://dats.treestats.net/dats/portal/files/16777217) | Get a file by ID from a DAT. By default the file is served as a single static pre-extracted R2 object (no block-chain traversal, no D1). Add `?source=dat` to read the original DAT via D1 metadata and block-chain traversal instead, or `?format=json` for a JSON representation of CharGen or SpellTable entries. | [`https://dats.treestats.net/dats/portal/files/16777217`](https://dats.treestats.net/dats/portal/files/16777217) |
| [`/dats/cell/landblocks/:block`](https://dats.treestats.net/dats/cell/landblocks/0xB34A) | Get the ACLB landblock bundle (terrain `…FFFF` + LandblockInfo `…FFFE`) for one landblock, served from the pre-extracted R2 bucket as `cell/landblocks/0x{block:04X}.bin`. `:block` accepts a 16-bit landblock id (`0xB34A`) or any full file id in the block (`0xB34AFFFF`). Only the cell DAT has bundles. | [`https://dats.treestats.net/dats/cell/landblocks/0xB34A`](https://dats.treestats.net/dats/cell/landblocks/0xB34A) |
| [`/dats/cell/envcells/:block`](https://dats.treestats.net/dats/cell/envcells/0xB34A) | Get the ACLB EnvCell bundle for one landblock, served as `cell/envcells/0x{block:04X}.bin`. `:block` accepts a landblock id (`0xB34A`) or any full cell id in it (`0x1E3033D`). Only the cell DAT has bundles. | [`https://dats.treestats.net/dats/cell/envcells/0xB34A`](https://dats.treestats.net/dats/cell/envcells/0xB34A) |
| [`/dats/world/landblock-instances/:block`](https://dats.treestats.net/dats/world/landblock-instances/0xB34A) | Get the **ACLI** landblock-instances file: the ACE world database's static props (fountains, doors, chests, lifestones, and similar) pre-resolved into one self-describing blob, served from the pre-extracted R2 bucket as `world/landblock-instances/0x{block:04X}.bin`. `:block` accepts a 16-bit landblock id (`0xB34A`) or any full file id in the block (`0xB34AFFFF`). | [`https://dats.treestats.net/dats/world/landblock-instances/0xB34A`](https://dats.treestats.net/dats/world/landblock-instances/0xB34A) |
| [`/dats/world/instance-index/:name`](https://dats.treestats.net/dats/world/instance-index/manifest.json) | Get the **ACVC** instance index the viewer browses: one row per static landblock instance with its name, landblock, Setup, WeenieType, MotionTable and ACLI ordinal, served from the pre-extracted R2 bucket as `world/instance-index/{name}`. Only `manifest.json` (mutable; revalidated) and `instance-index-{16 hex}.bin` (content-hashed; immutable) are served. The `.bin` is stored raw and brotli (`.bin.br`), and the precompressed copy is served with `Content-Encoding: br` because Cloudflare does not compress `application/octet-stream`: ~120 KB instead of 1.1 MB. | [`https://dats.treestats.net/dats/world/instance-index/manifest.json`](https://dats.treestats.net/dats/world/instance-index/manifest.json) |
| [`/dats/:dat/setups/:id`](https://dats.treestats.net/dats/portal/setups/0x02001B7F) | Get a precomputed **ACSP** setup pack: a Setup's entire render closure (Setup + GfxObjs + Surfaces + SurfaceTextures + Textures + Palettes) in one object, served from the pre-extracted R2 bucket as `portal/setups/0x{id:08X}.bin`. Only the portal DAT has setup packs. Generate with the ac-noclip `scripts/gen-setup-packs.ts`. | [`https://dats.treestats.net/dats/portal/setups/0x02001B7F`](https://dats.treestats.net/dats/portal/setups/0x02001B7F) |
| [`/icons`](https://dats.treestats.net/icons) | List icon metadata as paginated JSON Lines (default and maximum limit 1000) | [`https://dats.treestats.net/icons?limit=100&after=0`](https://dats.treestats.net/icons?limit=100&after=0) |
| [`/icons/:id`](https://dats.treestats.net/icons/26967) | Get icon as PNG | [`https://dats.treestats.net/icons/26967?scale=2`](https://dats.treestats.net/icons/26967?scale=2) |
| [`/setups/:id`](https://dats.treestats.net/setups/0x02000108) | Get a portal DAT Setup (0x02) by ID as raw binary, or a multipart/mixed bundle of the Setup and its GraphicsObject (0x01) dependencies via `?include=gfxobjs`. IDs accept decimal or hex (0x/0X) forms. Note: `fetch()` does not parse multipart/mixed responses automatically; consumers must parse the MIME boundary or use a MIME parser. | [`https://dats.treestats.net/setups/0x02000108`](https://dats.treestats.net/setups/0x02000108) |

### Pagination

The paginated JSON Lines endpoints use a uniform default and maximum page size of `1000`:
`/dats/:dat/files` and `/icons`. Use `after` with the last record ID, preferably from the
`X-Next-After` response header, for sequential pagination. `offset` remains available for
compatibility but becomes less efficient on later pages. `X-Limit` and `X-Offset` report the
applied page settings. Supplying both `after` and `offset` uses cursor pagination.

## Caching

Successful binary DAT, bundle, and setup-pack responses return:

```text
Cache-Control: public, max-age=31536000, immutable
```

DAT content and generated bundles are immutable for a given deployment/patch.
The URL is keyed by DAT id (or landblock / setup id), so a patch that reuses an
id must invalidate the cache (for example with a purge or a generation prefix)
when republishing.

## Generating landblock bundles

The `acdatservice` CLI turns the cell DAT into per-landblock **ACLB** bundle
objects. Each bundle packs one landblock's files into a single self-describing
blob, which collapses many tiny cell reads into one request per block. The
byte-level format is documented in [`docs/bundle-format.md`](./docs/bundle-format.md).

```sh
# Terrain (…FFFF) + LandblockInfo (…FFFE), one file per landblock.
DATS_DIR="$HOME/Dats" cargo run --bin acdatservice --features=generate -- \
  generate landblock -o generated/

# Every EnvCell (…01xx–…FExx), one file per landblock.
DATS_DIR="$HOME/Dats" cargo run --bin acdatservice --features=generate -- \
  generate envcell -o generated/
```

The DAT is read from `$DATS_DIR/client_cell_1.dat` (override with `--dat`).
Output is one file per landblock, written to `<out>/landblocks/0x<BLOCK>.bin` or
`<out>/envcells/0x<BLOCK>.bin`, where `<BLOCK>` is the full four-digit uppercase
hex landblock id. The subdirectory mirrors the R2 prefix used by the
`/dats/cell/landblocks/:block` and `/dats/cell/envcells/:block` routes, so
`generate … -o export/cell` followed by syncing `export/cell/` publishes the
bundles beside the per-id files in the extracted bucket. Pass `--block 0xB34A`
to generate a single block.

## Generating setup packs

A standalone Setup's render closure is 15–167 portal files (Setup, GfxObjs,
Surfaces, SurfaceTextures, Textures, Palettes). The **ACSP** setup pack collapses
that into one object per Setup, served by
`/dats/:dat/setups/:id` from `portal/setups/0x{id:08X}.bin`. The byte-level
format is documented in [`docs/setup-bundle-format.md`](./docs/setup-bundle-format.md).

The closure walk lives in ac-noclip (it needs the Setup/GfxObj/Surface parsers),
so generation is a TypeScript tool in that repo:

```sh
# In ac-noclip. Writes <out>/portal/setups/0x{id:08X}.bin for every renderable Setup.
DATS_DIR="$HOME/Dats" npx tsx scripts/gen-setup-packs.ts --out=export
# One Setup, or the first N of the catalog.
DATS_DIR="$HOME/Dats" npx tsx scripts/gen-setup-packs.ts --id=0x02001B7F --out=export
DATS_DIR="$HOME/Dats" npx tsx scripts/gen-setup-packs.ts --limit=100 --out=export
```

The output subdirectory mirrors the R2 prefix, so syncing `export/portal/setups/`
publishes the packs beside the per-id files in the extracted bucket (see
[Pre-extracting Files to R2](#pre-extracting-files-to-r2)).

## Generating and publishing landblock instances

The ACE world database places server-side objects — fountains, doors, chests,
lifestones, and similar static props — that are not in the client DATs. The
**ACLI** landblock-instances format pre-resolves each instance's model,
placement, and appearance offline into one small file per landblock, served by
`/dats/world/landblock-instances/:block` from the R2 key
`world/landblock-instances/0x{block:04X}.bin` for legacy callers. With
`?revision=spawns-v1`, it reads only
`world/landblock-instances-revisions/spawns-v1/0x{block:04X}.bin` (404 on a miss,
never stale fallback). See [immutable dataset revisions](./docs/acli-dataset-revisions.md)
for validation, exact staging commands, overwrite protection and rollback. The byte-level format is
documented in [`docs/landblock-instances-format.md`](./docs/landblock-instances-format.md).

Like ACSP setup packs, generation needs the ACE world database joins, so it is a
TypeScript tool in ac-noclip:

```sh
# In ac-noclip. Writes <out>/world/landblock-instances/0x{block:04X}.bin.
npx tsx scripts/gen-landblock-instances.ts --out=export
```

The output subdirectory mirrors the R2 prefix. Publish it with
`scripts/upload_landblock_instances.sh`, which mirrors
`export/world/landblock-instances/` (override with `SRC_DIR`) to
`s3://$BUCKET/world/landblock-instances/` using `rclone sync` (`BUCKET` is
required; preview with `DRYRUN=1`). This is **legacy mode**, requiring
`CONFIRM_LEGACY_FULL_TREE=1` and a verified complete export. For new datasets,
set `REVISION=spawns-v1` to select the isolated immutable revision prefix;
revision publication uses copy, never sync/delete.

Before touching R2, the script runs `scripts/check_acli_publish.mjs`. The guard
refuses to publish unless the deployed ac-noclip client's `/acli-layout.json`
marker advertises the current 60-byte record base **and every feature bit found
in the actual file headers**. Optional marker `decoderFeatureBits` defaults to
zero; appearance tails require mask `1`. The guard validates the sized appearance
payloads as well as the base records. See the
[format and offline demo](./docs/landblock-instances-format.md). Future appearance
data must use a new revision, never overwrite `spawns-v1`.

```sh
export AWS_ACCESS_KEY_ID=<token-id>
export AWS_SECRET_ACCESS_KEY=<token-secret>
export AWS_ENDPOINT_URL=https://<account-id>.r2.cloudflarestorage.com

# Optional: point at a preview client while testing the guard.
export CLIENT_LAYOUT_URL=https://noclip.treestats.net/acli-layout.json

BUCKET=treestats-acdats-extracted REVISION=spawns-v1 DRYRUN=1 \
  SRC_DIR=/path/to/ac-noclip/export/spawns-v1/world/landblock-instances \
  ./scripts/upload_landblock_instances.sh  # preview without changes
```

**Revision rollout order:** with a compatible decoding marker already available,
finish/audit and upload the revision first, deploy service revision support, then
deploy the revision-pinned client. Keep the original tree untouched. The full
ac-noclip order is documented in
[`docs/acli-rollout.md`](https://github.com/amoeba/ac-noclip/blob/main/docs/acli-rollout.md).

### Generating and publishing the instance index

The viewer's Instances list needs 66,667 rows of name/block/Setup/WeenieType/
MotionTable/ordinal. That catalog is too large to inline in the bundle (14.6 MB
of placements; 6.4 MB as JSON) but tiny as a typed-array index: `ACVC` v1 is
~1.1 MB raw and **~120 KB brotli**, and decodes with zero per-row allocation.

Generation needs the ACE world DB names (they live on the weenie class, so the
ACLI tree cannot supply them), so it is a TypeScript tool in ac-noclip that
shares the ACLI generator's row ordering — the index's `ordinal` column is the
record's position among a block's instances of that Setup in ACLI (GUID) order,
which is what `?instance=block:setup:nth` resolves against.

```sh
# In ac-noclip. Writes <out>/instance-index-{hash}.bin + manifest.json.
npx tsx scripts/gen-instance-index.ts --out=export/world/instance-index
npx tsx scripts/gen-instance-index.ts --verify-acli   # check ordinals against the tree
```

Publish with `scripts/upload_instance_index.sh`, which uploads the hashed file
as `immutable`, its brotli copy as `instance-index-{hash}.bin.br`, and the
manifest as `must-revalidate`. It uses `wrangler r2 object put --remote` (note:
plain `wrangler r2 object` commands default to the local simulated bucket):

```sh
BUCKET=treestats-acdats-extracted ./scripts/upload_instance_index.sh
DRYRUN=1 ./scripts/upload_instance_index.sh
```

A run with no generated files refuses to sync rather than deleting the prefix
(see [Pre-extracting Files to R2](#pre-extracting-files-to-r2) for the shared R2
setup).

## Development

Development involves using the wrangler CLI and a Cloudflare account with the correct resources setup.
I don't have a guide but please reach out if you'd like to contribute and want help.

Note that this crate must use the same version of the `worker` crate because of type sharing with asheron-rs.

## Pre-extracting Files to R2

The `/dats/:dat/files/:file_id` endpoint serves files from a pre-extracted R2 bucket
(`treestats-acdats-extracted`) as individual flat objects. This avoids block-chain traversal
on every request. Requests for files that have not been extracted and uploaded yet can opt
back into the legacy behavior with `?source=dat`, which reads the original DAT using the D1
index and block-chain traversal (and returns the `X-R2-Read-Count` header). A missing extracted
object is an HTTP `404` and is never resolved automatically from the original DAT.

Only `/dats/:dat/files/:file_id` reads from the extracted bucket. The other DAT-backed
routes — `/setups/:id` and `/icons/:id` — intentionally remain on the D1 metadata +
block-chain traversal path against the original DATs in `treestats-acdats`; they do not
read from `treestats-acdats-extracted`.

To populate the extracted bucket:

### 1. Export each DAT locally using asheron-rs

```sh
# Requires the asheron-rs repository with the dat-export and dat-tokio features.
# See https://github.com/amoeba/asheron-rs

# Portal DAT (~65k+ files)
cargo run --bin dat --features dat-export,dat-tokio -- export \
  --file client_portal.dat --output export/portal/ --raw

# Cell DAT
cargo run --bin dat --features dat-export,dat-tokio -- export \
  --file client_cell_1.dat --output export/cell/ --raw

# High-res DAT
cargo run --bin dat --features dat-export,dat-tokio -- export \
  --file client_highres.dat --output export/highres/ --raw

# Local English DAT
cargo run --bin dat --features dat-export,dat-tokio -- export \
  --file client_local_English.dat --output export/local_english/ --raw
```

This creates files like `export/portal/0x02000108.bin`, `export/highres/0x060043BE.bin`, etc.

Alternatively, `scripts/export_dats.sh` drives the `dat` CLI for you and writes to
`export/<dat>/` (defaulting to all DATs, or pick one with e.g. `EXPORT_HIGHRES=1`).

### 2. Upload to R2

Mirror each `export/<dat>/` directory into the extracted bucket with
`aws s3 sync --delete` — the rsync `--delete` equivalent. The bucket prefix
becomes an exact mirror of the local export, so objects that no longer exist
locally are removed. Create the bucket first if it does not exist:

```sh
npx wrangler r2 bucket create treestats-acdats-extracted
```

Then mirror each DAT (uses the R2 S3 API, so export your R2 access key, secret,
and account endpoint first):

```sh
export AWS_ACCESS_KEY_ID=<token-id>
export AWS_SECRET_ACCESS_KEY=<token-secret>
export AWS_ENDPOINT_URL=https://<account-id>.r2.cloudflarestorage.com

for dat in portal cell highres local_english; do
  aws s3 sync "export/$dat/" "s3://treestats-acdats-extracted/$dat/" \
    --delete \
    --endpoint-url "$AWS_ENDPOINT_URL" \
    --region "${AWS_DEFAULT_REGION:-auto}" \
    --content-type application/octet-stream \
    --only-show-errors
done
```

Or run `scripts/upload_exported_dats.sh`, a thin wrapper that checks the R2
environment and mirrors each non-empty `export/<dat>/` (`BUCKET` is required;
preview with `DRYRUN=1`). `--delete` requires an R2 token with Object Read &
Write (ListBucket + DeleteObject). The generated `export/world/landblock-instances/`
tree is published the same way with `scripts/upload_landblock_instances.sh` (see
[Generating and publishing landblock instances](#generating-and-publishing-landblock-instances)).

After upload, files are addressable at keys like `portal/0x06006957.bin`,
`cell/0x01000001.bin`, etc. The Worker reads these directly with a single
`EXTRACTED_BUCKET.get(key)` — no block-chain traversal, no D1 query.

## Deployment

### Updating Cloudflare D1

To update the index on D1, run

```sh
# Upload each DAT to R2. The object key must match the filename.
npx wrangler r2 object put treestats-acdats/client_portal.dat --remote --file client_portal.dat
npx wrangler r2 object put treestats-acdats/client_cell_1.dat --remote --file client_cell_1.dat
npx wrangler r2 object put treestats-acdats/client_highres.dat --remote --file client_highres.dat
npx wrangler r2 object put treestats-acdats/client_local_English.dat --remote --file client_local_English.dat

# Index each DAT you want to serve. The database type is inferred from the filename.
cargo run --bin create_index --features=index -- client_portal.dat client_cell_1.dat client_highres.dat client_local_English.dat
# this creates data/index.sqlite
sh scripts/sync_d1.sh
# this dumps the database we just created, converts it to .sql, and executes
# on cloudflare
```

Regenerate and sync the index before deploying Worker code that uses the materialized
`dats.file_count` column and covering file indexes.

### Deploy to Cloudflare Workers

```sh
npx wrangler deploy
```
