# ACLB per-landblock bundle format

Version 1. Status: **draft** — the format is implemented and unit-tested in
[`src/bundle.rs`](../src/bundle.rs) and written by `acdatservice generate`, but
no consumer reads it yet.

## Why

The client currently reads one DAT file per HTTP request. Most cell-DAT files
are tiny (94.5% are ≤ 512 B; a typical EnvCell is ~250 B), so a dungeon view
issues thousands of small reads. Grouping the files that belong to one
landblock into a single object turns that into a constant number of requests
per block, and gives each block a deterministic, immutable cache key.

Two bundles cover the cell DAT’s per-block files:

- **`landblock`** — terrain (`…FFFF`) plus LandblockInfo (`…FFFE`). This is the
  small, always-needed metadata for a block.
- **`envcell`** — every EnvCell (`…01xx`–`…FExx`) owned by the block. This is
  the large, on-demand part, kept separate so overworld streaming never
  over-fetches interiors.

The split follows the recommendation in the ac-noclip
[`dungeon-network-study`](https://github.com/amoeba/ac-noclip/blob/main/research/dungeon-network-study.md)
(§6, strategy S4).

## Conventions

- All integers are **little-endian**.
- File ids are unsigned 32-bit values of the form `(landblock << 16) | suffix`,
  e.g. terrain for block `0xB34A` is `0xB34AFFFF`.
- Offsets in this document are relative to the start of the file.
- A bundle contains files from exactly **one** landblock. Every entry id must
  have that landblock as its high 16 bits.

## Layout

### Header — 16 bytes

| Offset | Size | Type  | Field       | Notes                                              |
| -----: | ---: | :---- | :---------- | :------------------------------------------------- |
|  `0x00`|    4 | bytes | `magic`     | `41 43 4C 42` = ASCII `ACLB`                       |
|  `0x04`|    1 | `u8`  | `version`   | `1`                                                |
|  `0x05`|    1 | `u8`  | `kind`      | `1` = landblock, `2` = envcell                     |
|  `0x06`|    2 | `u16` | `reserved`  | must be `0`                                        |
|  `0x08`|    4 | `u32` | `landblock` | high word of every entry id; `0..=0xFFFF`          |
|  `0x0C`|    4 | `u32` | `count`     | number of entries                                  |

### Entry table — `count × 12` bytes

Immediately follows the header. One record per bundled file, in **ascending id
order**.

| Offset | Size | Type  | Field    | Notes                                            |
| -----: | ---: | :---- | :------- | :----------------------------------------------- |
|  `0x00`|    4 | `u32` | `id`     | full DAT file id                                 |
|  `0x04`|    4 | `u32` | `offset` | absolute file offset of this entry’s payload     |
|  `0x08`|    4 | `u32` | `length` | payload length in bytes                          |

The offset is redundant with the on-disk packing order, but it lets a reader
build an index in one pass and then seek directly to any payload without
summing preceding lengths (random access).

### Payloads

Immediately follow the entry table, packed back to back with **no padding** or
alignment. The first payload starts at `16 + 12 × count`. Every payload is
described by its entry’s absolute `offset` and `length`.

Total file size:

```
16 + (12 × count) + Σ length
```

A bundle with `count = 0` is valid (a 16-byte header) and means “this landblock
has none of this kind of file”.

## Invariants

A decoder MUST reject a file that violates any of these:

1. Fewer than 16 bytes.
2. `magic != "ACLB"`.
3. `version != 1`.
4. `kind` is not `1` or `2`.
5. `reserved != 0`.
6. `landblock > 0xFFFF`.
7. The entry table extends past the end of the file.
8. Any payload extends past the end of the file.
9. Any `id >> 16` differs from `landblock`.
10. Any `offset` points into the header or entry table
    (`offset < 16 + 12 × count`).
11. The payload ranges do not tile the bytes after the table exactly — that
    is, there is a gap, an overlap, or trailing bytes.

A decoder MUST NOT assume the entry ids are contiguous, sorted, or a fixed
size; it must walk the table. Encoders SHOULD write entries in ascending id
order.

## Entry membership

| Kind        | Value | Ids included                                              |
| :---------- | ----: | :-------------------------------------------------------- |
| `landblock` | `1`   | suffix `0xFFFE` (LandblockInfo) and `0xFFFF` (terrain)    |
| `envcell`   | `2`   | suffix in `0x0100..0xFF00` (EnvCell)                      |

Zero-length DAT entries are omitted. Terrain exists for all 255×255 landblocks;
LandblockInfo exists for only ~8% of them, so most `landblock` bundles contain
a single entry.

## Worked example

An `envcell` bundle for landblock `0x00AE` with two cells:

| id           | payload      |
| :----------- | :----------- |
| `0x00AE0100` | `DE AD`      |
| `0x00AE0101` | `BE EF 00`   |

```text
offset  bytes
0x0000  41 43 4C 42   magic "ACLB"
0x0004  01            version 1
0x0005  02            kind = envcell
0x0006  00 00         reserved
0x0008  AE 00 00 00   landblock = 0x000000AE
0x000C  02 00 00 00   count = 2
0x0010  00 01 AE 00   entry 0 id = 0x00AE0100
0x0014  28 00 00 00   entry 0 offset = 40
0x0018  02 00 00 00   entry 0 length = 2
0x001C  01 01 AE 00   entry 1 id = 0x00AE0101
0x0020  2A 00 00 00   entry 1 offset = 42
0x0024  03 00 00 00   entry 1 length = 3
0x0028  DE AD         entry 0 payload
0x002A  BE EF 00      entry 1 payload
```

One line:

```text
41 43 4C 42 01 02 00 00 AE 00 00 00 02 00 00 00
00 01 AE 00 28 00 00 00 02 00 00 00 01 01 AE 00
2A 00 00 00 03 00 00 00 DE AD BE EF 00
```

## Reference reader (pseudocode)

```text
read u32 magic; require magic == "ACLB"
read u8  version; require version == 1
read u8  kind;    require kind == 1 or 2
read u16 reserved; require reserved == 0
read u32 landblock; require landblock <= 0xFFFF
read u32 count

payload_start = 16 + count * 12
entries = []
repeat count times:
    read u32 id
    read u32 offset
    read u32 length
    require id >> 16 == landblock
    require payload_start <= offset
    require offset + length <= file_size
    entries[id] = bytes[offset .. offset + length]

# Offsets must tile the payload region exactly: no gaps, overlaps, or trailing
# bytes. This check is order-independent.
require ranges(entries).sorted() == tile(payload_start, file_size)
```

## Rust API

[`acdatservice::bundle`](../src/bundle.rs) is the shared implementation:

```rust
use acdatservice::bundle::{decode, encode, Bundle, BundleEntry, BundleKind};

let bundle = Bundle {
    kind: BundleKind::Envcell,
    landblock: 0x00AE,
    entries: vec![BundleEntry { id: 0x00AE_0100, data: vec![0xDE, 0xAD] }],
};
let bytes = encode(&bundle);
let parsed = decode(&bytes)?;
assert_eq!(parsed.entry(0x00AE_0100), Some(&[0xDE, 0xAD][..]));
```

## CLI

```sh
# All landblocks (one file per block) with bundled terrain + LandblockInfo.
DATS_DIR="$HOME/Dats" acdatservice generate landblock -o generated/

# All EnvCells, one file per block.
DATS_DIR="$HOME/Dats" acdatservice generate envcell -o generated/

# One block only.
DATS_DIR="$HOME/Dats" acdatservice generate envcell --block 0xB34A -o generated/
```

Output files are named `0x<BLOCK>.landblock.bin` and `0x<BLOCK>.envcell.bin`,
where `<BLOCK>` is the full four-digit uppercase hexadecimal landblock id
(e.g. `0xB34A.envcell.bin`).

## Versioning

Any layout change — new field, different framing, different membership — MUST
increment `version`. Decoders MUST reject unknown versions rather than guess.
There is no negotiation; a bundler and its readers ship together.

## Non-goals

- No compression is applied by the format. Compress the whole object at rest /
  on the wire (gzip/brotli) if desired.
- This format only covers cell-DAT, per-landblock files. Portal DAT graphics
  (Setups, GfxObjs, textures) are out of scope.
