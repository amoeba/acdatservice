# @amoeba/acdatservice-client

Browser-safe codecs for custom acdatservice contracts. This package owns the
TypeScript side of the ACLI, ACLB, ACSP, ACAP area-pack, ACVC instance-index,
and multipart response formats. The Rust service implementations remain in
this repository and are checked against shared byte fixtures.

Import the complete codec API from `@amoeba/acdatservice-client`, or use a
subpath such as `/multipart` or `/acli`. It has no HTTP client, cache, scheduler,
renderer, or Node runtime dependency.

## Dependencies

`@amoeba/libac` is a peer dependency (`^0.3.0`). A pinned development dependency supplies the same types during builds; downstream
applications supply a single compatible native library through the peer.
The package imports types only:

- `Appearance extends ObjDesc` and adds only the service-specific held
  `attachments` traversal; parts, textures, palettes, and `paletteBaseId` are
  the canonical native `ObjDesc` definitions.
- No runtime libac import is emitted, so the browser bundle stays independent.

## Formats

- `multipart`: binary-safe multipart/mixed encoder and metadata-rich parser (`fileId`, `contentType`,
  `filename`, `contentLocation`, owned `body`).
- `acli`: per-landblock instance records, including appearance tails and held
  attachments. Current 60-byte records plus legacy 56/52-byte read support.
- `aclb`, `acsp`: landblock and setup packs, byte-compatible with Rust.
- `area-pack`, `instance-index`, `acli-layout`: catalog and layout contracts.
- `dat-routing`: service archive names plus `datFor`. The ID-based route is a
  clearly marked historical fallback; pass an explicit `archive` (or a known
  `kind`) whenever available. Native ID spaces overlap, so no ID is
  universally routable.

## Verification

```sh
npm run build:client       # emits client/dist
npm run check:browser:client # bundles root + /multipart, runs without Node/Three/test globals
npm run test:client        # unit + shared Rust fixture compatibility
npm run verify:client      # all of the above
```
