# acdatservice

Service for dynamically serving Asheron's Call Cell and Portal DAT resources.
Built with Cloudflare Workers, R2, and D1.

## Status

- Icons
  - Get icon as PNG, scale from 1x-8x
  - Example: <https://dats.treestats.net/icons/26967?scale=2>

## Development

Development involves using the wrangler CLI and a Cloudflare account with the correct resources setup.
I don't have a guide but please reach out if you'd like to contribute and want help.

Note that this crate must use the same version of the `worker` crate because of type sharing with libac-rs.
