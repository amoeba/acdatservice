# acdatservice

Service for dynamically serving Asheron's Call Cell and Portal DAT resources.
Built with Cloudflare Workers, R2, and D1.

## API Routes

| Route | Description | Example |
|-------|-------------|---------|
| [`/`](https://dats.treestats.net/) | OpenAPI specification | [`https://dats.treestats.net/`](https://dats.treestats.net/) |
| [`/files`](https://dats.treestats.net/files) | List all file IDs | [`https://dats.treestats.net/files`](https://dats.treestats.net/files) |
| [`/icons`](https://dats.treestats.net/icons) | List all icon IDs | [`https://dats.treestats.net/icons`](https://dats.treestats.net/icons) |
| [`/icons/:id`](https://dats.treestats.net/icons/26967) | Get icon as PNG | [`https://dats.treestats.net/icons/26967?scale=2`](https://dats.treestats.net/icons/26967?scale=2) |

## Development

Development involves using the wrangler CLI and a Cloudflare account with the correct resources setup.
I don't have a guide but please reach out if you'd like to contribute and want help.

Note that this crate must use the same version of the `worker` crate because of type sharing with asheron-rs.

## Deployment

### Updating Cloudflare D1

To update the index on D1, run

```sh
cargo run --bin create_index --features=index -- client_portal.dat
# this creates data/index.sqlite
sh scripts/sync_d1.sh
# this dumps the database we just created, converts it to .sql, and executes
# on cloudflare
```

### Deploy to Cloudflare Workers

```sh
npx wrangler deploy
```
