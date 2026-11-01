# ACLI landblock-instances format

Version **1**, current fixed record base **60 bytes**. Implemented in
[`src/landblock_instances.rs`](../src/landblock_instances.rs) and
[`src/appearance.rs`](../src/appearance.rs); the matching TypeScript codecs are
ac-noclip's `src/ac/acli.ts` and `src/ac/appearance.ts`.

ACLI pre-resolves ACE world database placements and appearance offline into one
file per landblock. The Worker serves these bytes unchanged from R2 through
`/dats/world/landblock-instances/:block`; it does not decode/re-encode files or
resolve appearance. Dataset selection, immutable caching and rollback are
specified in [immutable dataset revisions](./acli-dataset-revisions.md).

## Conventions

- All integers and floats are **little-endian**; operation arrays have no padding.
- One file describes exactly one landblock. Record order and operation order are
  meaningful: retain authored/layer order; later appearance changes win.
- DAT id families: Setup `0x02`, GfxObj `0x01`, Surface `0x08`, SurfaceTexture
  `0x05`, Texture `0x06`, Palette `0x04`. Codecs retain ids verbatim.
- Palette offsets/lengths are **blocks of eight colors**, not individual colors
  or bytes. Encoding/decoding does not multiply them by eight.

## Header — 16 bytes

| Offset | Size | Type | Field | Notes |
| ---: | ---: | :--- | :--- | :--- |
| `0x00` | 4 | bytes | magic | ASCII `ACLI` |
| `0x04` | 1 | `u8` | version | `1` |
| `0x05` | 1 | `u8` | kind | `1` = landblock instances |
| `0x06` | 2 | `u16` | featureBits | `0`, `ACLI_FEATURE_APPEARANCE = 0x0001`, or mask `3` with attachments |
| `0x08` | 4 | `u32` | landblock | `0..=0xFFFF` |
| `0x0C` | 4 | `u32` | instanceCount | record count |

Bytes 6–7 were previously reserved zero. **Mask value 1 (bit 0)** now enables
appearance tails. This does not change version 1 or the 60-byte record base.
**Mask value 2 (bit 1)** enables held attachments within those tails and requires
mask value 1: `2` alone is invalid, `3` is valid. Known feature mask is `3`.
All other feature bits are unknown and must be rejected, not ignored. Existing
`appearance-v1` files (mask 1) remain readable without reinterpretation.

## Records — `instanceCount` records, back to back

| Offset | Size | Type | Field | Notes |
| ---: | ---: | :--- | :--- | :--- |
| `0x00` | 4 | `u32` | setupId | model id |
| `0x04` | 12 | 3×`f32` | originX/Y/Z | landblock-local AC coordinates |
| `0x10` | 16 | 4×`f32` | quatW/X/Y/Z | orientation |
| `0x20` | 4 | `f32` | scale | DefaultScale, default `1.0` |
| `0x24` | 4 | `u32` | paletteId | legacy sub-palette, `0` = none |
| `0x28` | 2 | `u16` | paletteOffset | legacy palette range start, blocks of eight |
| `0x2A` | 2 | `u16` | paletteLength | legacy palette range length, blocks of eight |
| `0x2C` | 1 | `u8` | weenieType | ACE WeenieType |
| `0x2D` | 1 | `u8` | flags | placement flags, retained verbatim |
| `0x2E` | 2 | `u16` | textureCount (`T`) | legacy texture override count |
| `0x30` | 4 | `u32` | motionTableId | MotionTable (`0x09`), `0` = none |
| `0x34` | 4 | `u32` | physicsScriptId | PhysicsScript (`0x33`), `0` = none |
| `0x38` | 4 | `u32` | guid | source instance GUID, `0` = absent |
| `0x3C` | `6 × T` | | legacy overrides | each `{u16 partIndex, u32 newTextureId}` |
| after overrides | 4 | `u32` | appearanceByteLength (`A`) | **only if feature mask 1 is set** |
| after length | `A` | bytes | appearance | zero length means absent |

When mask 1 is set, **every record** has its own length word, including records
without appearance (`A = 0`). No length word exists when mask 1 is clear.
A present but empty appearance is a **12-byte** payload, distinct from absence.
Mask 3 uses the same outer sized-tail layout; attachments are inside `A`.

```
featureBits = 0: 16 + Σ (60 + 6 × T)
featureBits = 1: 16 + Σ (60 + 6 × T + 4 + A)
```

An empty landblock is a valid 16-byte header. Encoders derive featureBits from
whether any record has a present appearance and any root appearance has
attachments, matching TypeScript. An explicitly flagged file may re-encode with
fewer feature bits if those features are unused.
The decoder does not guess legacy layouts when a feature flag identifies tails.
The Rust codec writes/reads only the current 60-byte base; accepting old 52/56
layouts is not required. Old files remain servable because routing is raw bytes.

## Appearance payload

This is a resolved binary ObjDesc operation list, not JSON or raw database rows.
It follows the legacy overrides without replacing or reinterpreting them.

| Offset | Size | Type | Field |
| ---: | ---: | :--- | :--- |
| `0x00` | 4 | `u32` | paletteBaseId (`0` = absent) |
| `0x04` | 2 | `u16` | partCount (`P`) |
| `0x06` | 2 | `u16` | textureCount (`X`) |
| `0x08` | 2 | `u16` | paletteCount (`C`) |
| `0x0A` | 2 | `u16` | attachmentCount (`H`) with mask 3; reserved zero with mask 1 |
| `0x0C` | `6 × P` | | parts: `{u16 partIndex, u32 gfxObjId}` |
| after parts | `10 × X` | | textures: `{u16 partIndex, u32 oldTextureId, u32 newTextureId}` |
| after textures | `8 × C` | | palettes: `{u32 paletteId, u16 offset, u16 length}` |

With feature1 only, the payload length must be **exactly
`12 + 6×P + 10×X + 8×C`**. With mask 3, attachment records follow these operations.
Part changes
replace a Setup part's GfxObj; texture changes are conditional on matching the
old SurfaceTexture id within that part. Palette ranges remain in blocks of eight.
Codecs must preserve duplicates and authored order, not sort or deduplicate them.
`appearance::encode` rejects operation counts above `u16::MAX`. The outer Rust
`landblock_instances::encode` retains its infallible `Vec<u8>` API and panics
rather than silently wrapping an unencodable count or accepting invalid attachments.

### Held attachment records (mask 3 only)

`Appearance.attachments` is optional in TypeScript and an empty-by-default
`Vec<HeldAttachment>` in Rust. Each of the `H` children follows all operation
arrays, in authored order, without padding:

| Offset | Size | Type | Field |
| ---: | ---: | :--- | :--- |
| `0x00` | 4 | `u32` | setupId, must be `0x02xxxxxx` |
| `0x04` | 2 | `u16` | parentLocation: `1`, `2`, `3`, or `8` |
| `0x06` | 2 | `u16` | placementId: `1`, `2`, `3`, or `6` |
| `0x08` | 4 | `f32` | scale, finite and strictly positive |
| `0x0C` | 4 | `u32` | child appearanceByteLength (`N`), zero = absent |
| `0x10` | `N` | bytes | nested appearance, same global feature mode |

A root appearance has depth zero. At most **4 attachment edges** and **32 total
attachment nodes per root** are allowed, including leaves without child
appearance. The node budget is shared across all siblings and descendants, not
reset for each nested payload. Depth-4 appearances cannot hold more attachments.
An empty child appearance is 12 bytes and distinct from absence. Duplicate Setup
ids at different nodes are legal; there is no global Setup-id cycle ban.
TypeScript rejects cyclic in-memory object graphs; owned Rust values cannot
contain such cycles. Every bounded payload must be consumed exactly.

Standalone `appearance::encode`/`decode` enable attachments by default, matching
TypeScript. ACLI always passes explicit header mode through
`encode_with_attachments`/`decode_with_attachments`, so feature1 still rejects a
nonzero formerly-reserved word. Encoders reject depth/count and identifier/scale
violations as well as decoders; Rust validates depth before recursive encoding.

The renderer, not this byte-serving service, resolves parent holding-location
part/frame and child placement frames from DAT Setups. No transforms, renderer
fallbacks, animation, equipment selection, or client export changes are implied
by this codec. Parent owns export integration and rollout.

## Validation and publication contract

Reject short/bad headers, unsupported versions/kinds/features, out-of-range
landblocks, truncated records/overrides/lengths/payloads, nonzero appearance
reserved words in feature1, inconsistent operation lengths/counts, invalid
attachment identifiers/scales, depth/node budget excess, or trailing bytes. Do not
allocate from an unchecked outer `instanceCount` or retry malformed appearance
files as an older record base.

The publish guard inspects **actual headers** and structurally validates every
file. It unions required feature bits across the tree (feature-free and flagged
files may coexist) and requires the deployed marker to accept both base 60 and
that entire mask. Marker format/version/kind are unchanged:

```json
{
  "format": "ac-noclip/acli-layout",
  "version": 1,
  "kind": 1,
  "recordBaseBytes": 60,
  "decoderRecordBaseBytes": [60, 56, 52],
  "decoderFeatureBits": 3
}
```

`decoderFeatureBits` is an optional uint16; omitted/null defaults to `0`, matching
ac-noclip's parser. The guard requires
`(decoderFeatureBits & requiredFeatureBits) === requiredFeatureBits`. Advertising
base 60 alone **does not** imply support for appearance. An export manifest may
record `featureBits: 3`, but it is not trusted instead of reading the actual files.
Unknown header features or malformed tails fail before publisher commands or a
revision reservation, even when the marker claims to support those bits.

No data is uploaded by these codec changes. Attachment data needs a **new
immutable dataset revision**: do not overwrite `appearance-v1` or `spawns-v1`.
The complete production `appearance-v1` tree remains available for rollback.
Parent coordinates
compatible client/marker availability, generation, publication and rollout. The
format marker guard and immutable publication safeguards both remain mandatory.

## Offline demo / cross-language fixture

```sh
cargo run --example acli_appearance
cargo test --lib --tests --examples
node --test scripts/check_acli_publish.test.mjs scripts/upload_landblock_instances.test.mjs
```

The demo prints 262 bytes as hex and verifies a Rust roundtrip: three synthetic
records with a nonzero GUID, a legacy override plus full appearance, an absent
appearance, and a present empty appearance. `tests/fixtures/acli-appearance.hex`
was independently encoded by the parent TypeScript codec with the same input.
Rust tests decode and re-encode it byte-for-byte, validate every truncation and
malformed tail, and retain operation order and palette block units. The example
can be compared directly with that fixture. Everything is local and synthetic;
no database, R2, browser/e2e, upload or deployment is needed.

`tests/fixtures/acli-attachments.hex` was independently generated by the TS
encoder from `acli-attachments.json`: absent, empty, and nested held appearances,
with a non-unit shield scale and a nested child without appearance. Rust checks
all fields and re-encodes the exact bytes. Rust/guard/publisher tests also cover
truncation, invalid attachment fields, unknown or dependent feature bits,
feature1 compatibility, inclusive depth4/node32 limits and cross-sibling budget
abuse. Publisher mocks verify mask3 is required **before any publisher call or
revision reservation**, and existing immutable checks remain unchanged.
