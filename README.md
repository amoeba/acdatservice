# acdatservice

Service for dynamically serving Asheron's Call Cell and Portal DAT resources.
Built with Cloudflare Workers, R2, and D1.

## API Routes

| Route | Description | Example |
|-------|-------------|---------|
| [`/`](https://dats.treestats.net/) | OpenAPI specification | [`https://dats.treestats.net/`](https://dats.treestats.net/) |
| [`/dats`](https://dats.treestats.net/dats) | List available DATs with file counts, sizes, and sha256 hashes | [`https://dats.treestats.net/dats`](https://dats.treestats.net/dats) |
| [`/dats/:dat/files`](https://dats.treestats.net/dats/portal/files) | List file metadata as JSON Lines (paginated, default and maximum limit 1000). DAT names: `portal`, `cell`, `highres`, `local_english` | [`https://dats.treestats.net/dats/portal/files?limit=100&offset=0`](https://dats.treestats.net/dats/portal/files?limit=100&offset=0) |
| [`/dats/:dat/files/:file_id`](https://dats.treestats.net/dats/portal/files/16777217) | Get a file by ID from a DAT. File is served as a single static R2 object (no block-chain traversal, no D1). Add `?format=json` for a JSON representation of CharGen or SpellTable entries. | [`https://dats.treestats.net/dats/portal/files/16777217`](https://dats.treestats.net/dats/portal/files/16777217) |
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

Successful binary DAT file responses and setup bundles return:

```text
Cache-Control: public, max-age=86400, s-maxage=604800, stale-while-revalidate=86400
```

Browsers can reuse an asset for one day. Shared caches may retain it for one week and serve
the prior response for one additional day while refreshing it. The service intentionally does
not mark these URLs `immutable`: publishing a new DAT can change bytes for an existing file ID.

## Development

Development involves using the wrangler CLI and a Cloudflare account with the correct resources setup.
I don't have a guide but please reach out if you'd like to contribute and want help.

Note that this crate must use the same version of the `worker` crate because of type sharing with asheron-rs.

## Pre-extracting Files to R2

The `/dats/:dat/files/:file_id` endpoint serves files from a pre-extracted R2 bucket
(`treestats-acdats-extracted`) as individual flat objects. This avoids block-chain traversal
on every request. To populate this bucket:

### 1. Export each DAT locally using asheron-rs

```sh
# Requires the asheron-rs repository with the dat-export and dat-tokio features.
# See https://github.com/amoeba/asheron-rs

# Portal DAT (~65k+ files)
cargo run --bin dat --features dat-export,dat-tokio -- export \
  --file client_portal.dat --output export-portal/ --raw

# Cell DAT
cargo run --bin dat --features dat-export,dat-tokio -- export \
  --file client_cell_1.dat --output export-cell/ --raw

# High-res DAT
cargo run --bin dat --features dat-export,dat-tokio -- export \
  --file client_highres.dat --output export-highres/ --raw

# Local English DAT
cargo run --bin dat --features dat-export,dat-tokio -- export \
  --file client_local_English.dat --output export-local-english/ --raw
```

This creates files like `export-portal/Texture/0x06006957.bin`, `export-portal/0x02000108.bin`, etc.

### 2. Upload to R2

Flatten the file-type subdirectories into a single namespace per DAT and upload to the
extracted bucket:

```sh
# Flatten upload helper — strips the Texture/, CharGen/, SpellTable/ subdirectories
# so every file lands at <dat_name>/0x<id>.bin.
upload_dat() {
  local dat_name=$1
  local export_dir=$2
  for f in $(find "$export_dir" -name '*.bin'); do
    base=$(basename "$f")
    aws s3 cp "$f" "s3://treestats-acdats-extracted/$dat_name/$base" \
      --endpoint-url <YOUR_R2_ENDPOINT> \
      --content-type application/octet-stream
  done
}

upload_dat portal   export-portal
upload_dat cell     export-cell
upload_dat highres  export-highres
upload_dat local_english export-local-english
```

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
