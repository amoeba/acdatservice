# acdatservice

Service for dynamically serving Asheron's Call Cell and Portal DAT resources.
Built with Cloudflare Workers, R2, and D1.

## API Routes

| Route | Description | Example |
|-------|-------------|---------|
| [`/`](https://dats.treestats.net/) | OpenAPI specification | [`https://dats.treestats.net/`](https://dats.treestats.net/) |
| [`/dats`](https://dats.treestats.net/dats) | List available DATs with file counts, sizes, and sha256 hashes | [`https://dats.treestats.net/dats`](https://dats.treestats.net/dats) |
| [`/dats/:dat/files`](https://dats.treestats.net/dats/portal/files) | List file metadata as JSON Lines (paginated, default and maximum limit 1000). DAT names: `portal`, `cell`, `highres`, `local_english` | [`https://dats.treestats.net/dats/portal/files?limit=100&offset=0`](https://dats.treestats.net/dats/portal/files?limit=100&offset=0) |
| [`/dats/:dat/files/:file_id`](https://dats.treestats.net/dats/portal/files/16777217) | Get a file by ID from a DAT | [`https://dats.treestats.net/dats/portal/files/16777217`](https://dats.treestats.net/dats/portal/files/16777217) |
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
