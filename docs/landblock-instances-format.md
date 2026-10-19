# ACLI landblock-instances format

Version 2. Status: **draft** — implemented and unit-tested in
[`src/landblock_instances.rs`](../src/landblock_instances.rs); written by the
ac-noclip `scripts/gen-landblock-instances.ts` tool and served by
`/dats/world/landblock-instances/:block`.

## Why

The ACE **world database** (`ace_world_patches`) places server-side objects
(`landblock_instance`) that are *not* in the client DATs: the Meeting Hall mana
pool, town fountains, doors, portals, chests, lifestones, and similar static
props. This viewer only reads DAT-authored `Stab`s, so those objects are
missing. ACLI pre-resolves each instance's model + placement + appearance
(joining the `weenie` / `weenie_properties_*` tables offline) into one small
file per landblock, so the client can draw them without any database access.

## Conventions

- All integers are **little-endian**.
- File ids / DAT ids are the usual `0x` families: Setup `0x02`, GfxObj `0x01`,
  Surface `0x08`, SurfaceTexture `0x05`, Texture `0x06`, Palette `0x04`.
- A file describes exactly **one** landblock.
- Records are variable length and written in a deterministic order (the
  generator writes them sorted by source `guid`).

## Layout

### Header — 16 bytes

| Offset | Size | Type  | Field           | Notes                                      |
| -----: | ---: | :---- | :-------------- | :----------------------------------------- |
| `0x00` |    4 | bytes | `magic`         | `41 43 4C 49` = ASCII `ACLI`               |
| `0x04` |    1 | `u8`  | `version`       | `2`                                        |
| `0x05` |    1 | `u8`  | `kind`          | `1` = landblock instances                  |
| `0x06` |    2 | `u16` | `reserved`      | must be `0`                                |
| `0x08` |    4 | `u32` | `landblock`     | 16-bit block id (`0..=0xFFFF`)             |
| `0x0C` |    4 | `u32` | `instanceCount` | number of records                          |

### Records — `instanceCount` records, back to back

Each record is **52 bytes** plus `6 × textureCount` override bytes.

| Offset | Size      | Type  | Field                              | Notes |
| -----: | --------: | :---- | :--------------------------------- | :---- |
| `0x00` |         4 | `u32` | `setupId`                          | the `0x0200xxxx` model id |
| `0x04` |        12 | 3×`f32` | `originX`, `originY`, `originZ`  | landblock-local AC coords |
| `0x10` |        16 | 4×`f32` | `quatW`, `quatX`, `quatY`, `quatZ` | orientation |
| `0x20` |         4 | `f32` | `scale`                            | weenie `DefaultScale`, `1.0` when absent |
| `0x24` |         4 | `u32` | `paletteId`                        | sub-palette id, `0` = none |
| `0x28` |         2 | `u16` | `paletteOffset`                    | ×8 when applied |
| `0x2A` |         2 | `u16` | `paletteLength`                    | ×8 when applied |
| `0x2C` |         1 | `u8`  | `weenieType`                       | ACE WeenieType, for client filtering |
| `0x2D` |         1 | `u8`  | `flags`                            | bit0 = `is_Link_Child`; rest reserved |
| `0x2E` |         2 | `u16` | `textureCount` (`T`)               | number of texture overrides |
| `0x30` |         4 | `u32` | `motionTableId`                    | weenie `MotionTable` (0x09) DAT id, `0` = none |
| `0x34` | `6 × T`   |  | overrides                          | `u16 partIndex`, `u32 newTextureId` |

Total file size:

```
16 + Σ (52 + 6 × T)
```

A file with `instanceCount = 0` is a valid 16-byte file meaning "this landblock
has no instances".

### Appearance semantics (from the ACE world DB)

- `setupId` = `weenie_properties_d_i_d.value` where `type = 1`.
- `motionTableId` = `weenie_properties_d_i_d.value` where `type = 2`
  (`PropertyDataId.MotionTable`), `0` = none. The viewer uses it to animate
  doors, portals, and creatures.
- `scale` = `weenie_properties_float.value` where `type = 39` (`DefaultScale`),
  else `1.0`.
- palette override = `weenie_properties_palette (sub_Palette_Id, offset,
  length)`, applied to the base palette at `offset × 8` for `length × 8`
  colors (ACE `SubPalette` semantics).
- texture override = `weenie_properties_texture_map (index, old_Id, new_Id)`
  where `index` is the **Setup part index**; within that part, surfaces whose
  texture id equals `old_Id` are replaced by `new_Id`.

## Membership

The generator defaults to **static** rows (`landblock_instance.is_Link_Child =
0`), which is ~67k of ~365k rows; the rest are house hooks and generator
spawns. `weenieType` is carried so the client can further filter (e.g. drop
HotSpot `13`, Generator `24`, Creature `10`).

## Invariants

A decoder MUST reject a file that violates any of these:

1. Fewer than 16 bytes.
2. `magic != "ACLI"`.
3. `version != 2`.
4. `kind != 1`.
5. `reserved != 0`.
6. `landblock > 0xFFFF`.
7. Any record or override runs past the end of the file.
8. Fewer than `instanceCount` records, or trailing bytes after the last record.

## Versioning

Any layout change MUST increment `version`. Encoders always write the current
version (`2`); decoders MUST reject any other version rather than guess. There
is no backward-compatibility window: ACLI is not released software, so the
generator, the service, and the client move together.

## Worked header example

Block `0x0125`, 2 records, no overrides:

```text
offset  bytes
0x0000  41 43 4C 49   magic "ACLI"
0x0004  02            version 2
0x0005  01            kind = 1
0x0006  00 00         reserved
0x0008  25 01 00 00   landblock = 0x00000125
0x000C  02 00 00 00   instanceCount = 2
0x0010  ...           record 0 (52 bytes)
0x0044  ...           record 1 (52 bytes)
```
