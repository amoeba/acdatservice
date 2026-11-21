/**
 * ACLI per-landblock landblock-instances format — TypeScript reference.
 *
 * Browser-safe counterpart of the Rust decoder in
 * `acdatservice/src/landblock_instances.rs`. The authoritative specification is
 * `acdatservice/docs/landblock-instances-format.md`. Written by
 * `scripts/gen-landblock-instances.ts` (offline, from the ACE world DB) and
 * served from `/dats/world/landblock-instances/0x{block:04X}`.
 *
 * Each record ends with `motionTableId` and `physicsScriptId` (the weenie's
 * `PropertyDataId.MotionTable` and `PropertyDataId.PhysicsScript`) so the
 * viewer can animate instances and discover their physics effects. ACLI is
 * pre-release: layout changes are made in place (no version bump) and the
 * generator, service and client are updated together.
 *
 * This module has no Node dependencies so the browser can decode instances too.
 */

import { encodeAppearance, decodeAppearance, type Appearance } from "./appearance.js";

/** Header feature bit: records have a u32-sized resolved appearance tail after
 * legacy texture overrides. Zero length means no appearance for that record. */
export const ACLI_FEATURE_APPEARANCE = 0x01;
/** Appearance's reserved u16 becomes attachmentCount; requires feature 1. */
export const ACLI_FEATURE_ATTACHMENTS = 0x02;
export const ACLI_HEADER_LEN = 16;
/** Fixed record base through `guid`; texture overrides follow it. */
export const ACLI_RECORD_LEN = 60;
export const ACLI_OVERRIDE_LEN = 6;
export const ACLI_VERSION = 1;
export const ACLI_KIND_LANDBLOCK_INSTANCES = 1;

/** `flags` bit0: the row is a link child (generator/child spawn or house hook). */
export const ACLI_FLAG_LINK_CHILD = 0x01;
/** Resolved world-space generator output, not an unprocessed link-child row. */
export const ACLI_FLAG_GENERATED = 0x02;
/** ACLI transport bit for resolved AC NoDraw. This is NOT PhysicsState.NoDraw
 * (0x20): flags here belong to our exported placement format. */
export const ACLI_FLAG_NO_DRAW = 0x04;

/** One per-Setup-part texture replacement (`newTextureId` of `partIndex`). */
export interface TextureOverride {
  /** Setup part index the override applies to. */
  partIndex: number;
  /** DAT Texture/GfxObj id (0x06xxxxxx / 0x01xxxxxx) to use for that part. */
  newTextureId: number;
}

/** One placed object. All fields are the raw decoded values from the file. */
export interface LandblockInstance {
  /** `0x0200xxxx` Setup id. */
  setupId: number;
  originX: number;
  originY: number;
  originZ: number;
  /** Orientation quaternion (w, x, y, z). */
  quatW: number;
  quatX: number;
  quatY: number;
  quatZ: number;
  /** Uniform scale; `1.0` when the source weenie had no DefaultScale row. */
  scale: number;
  /** Sub-palette id (`0x04xxxxxx`), or `0` for none. */
  paletteId: number;
  /** Sub-palette offset; the client multiplies by 8 when applying. */
  paletteOffset: number;
  /** Sub-palette length; the client multiplies by 8 when applying. */
  paletteLength: number;
  /** ACE WeenieType, carried so the client can filter (HotSpot, Generator, …). */
  weenieType: number;
  /** Bitfield; see `ACLI_FLAG_LINK_CHILD` and `ACLI_FLAG_GENERATED`. */
  flags: number;
  /**
   * Weenie `PropertyDataId.MotionTable` (`0x09xxxxxx`), or `0` when the weenie
   * has none. Drives the default animation state.
   */
  motionTableId: number;
  /**
   * Weenie `PropertyDataId.PhysicsScript` (type 30), or `0` when absent. This
   * raw value may be a PlayScript enum or a PhysicsScript/PhysicsScriptTable id.
   */
  physicsScriptId: number;
  /** Per-part texture overrides, in file order. */
  textureOverrides: TextureOverride[];
  /** Resolved initial ObjDesc; when present, supersedes the legacy single
   * palette and part-wide texture fields. Shared by static/generated spawns. */
  appearance?: Appearance;
  /**
   * ACE weenie GUID — the instance's identity in the world database, and the
   * stable key for a link (Setup ids repeat inside a landblock).
   *
   * `undefined` (encoded as `0`) means the record came from a layout without
   * one: the 56-byte published tree, or a synthesized record such as a curated
   * catalog entry.
   */
  guid?: number;
}

export interface LandblockInstances {
  /** 16-bit block id (`0..=0xFFFF`). */
  landblock: number;
  /** Records, in file (generator: guid) order. */
  instances: LandblockInstance[];
}

/** True when `bytes` starts with the four ACLI magic bytes. */
export function hasAcliMagic(bytes: Uint8Array): boolean {
  return (
    bytes.byteLength >= 4 &&
    bytes[0] === 0x41 &&
    bytes[1] === 0x43 &&
    bytes[2] === 0x4c &&
    bytes[3] === 0x49
  );
}

/** True only when the full header (magic, version, kind) matches ACLI. */
export function isAcli(bytes: Uint8Array): boolean {
  return (
    hasAcliMagic(bytes) &&
    bytes.byteLength >= ACLI_HEADER_LEN &&
    bytes[4] === ACLI_VERSION &&
    bytes[5] === ACLI_KIND_LANDBLOCK_INSTANCES
  );
}

/** True when a decoded instance is a link child. */
export function isLinkChild(instance: LandblockInstance): boolean {
  return (instance.flags & ACLI_FLAG_LINK_CHILD) !== 0;
}

/**
 * Encode one landblock's instances. Records are written in the order given, so
 * the generator must sort by source guid first for byte-for-byte determinism.
 * The encoding is the exact inverse of `decodeAcli`.
 */
export function encodeAcli(
  landblock: number,
  instances: readonly LandblockInstance[],
): Uint8Array {
  if (!Number.isInteger(landblock) || landblock < 0 || landblock > 0xffff) {
    throw new Error(`ACLI: landblock out of range: ${landblock}`);
  }
  if (instances.length > 0xffffffff) {
    throw new Error(`ACLI: too many instances: ${instances.length}`);
  }

  const hasAppearance = instances.some(instance => instance.appearance !== undefined);
  const hasAttachments = instances.some(instance => (instance.appearance?.attachments?.length ?? 0) > 0);
  const appearances = instances.map(instance => instance.appearance ? encodeAppearance(instance.appearance, hasAttachments) : undefined);
  let size = ACLI_HEADER_LEN;
  for (const [index, instance] of instances.entries()) {
    if (instance.textureOverrides.length > 0xffff) {
      throw new Error(`ACLI: too many texture overrides: ${instance.textureOverrides.length}`);
    }
    size += ACLI_RECORD_LEN + instance.textureOverrides.length * ACLI_OVERRIDE_LEN;
    if (hasAppearance) size += 4 + (appearances[index]?.length ?? 0);
  }

  const out = new Uint8Array(size);
  const dv = new DataView(out.buffer);
  out[0] = 0x41; // 'A'
  out[1] = 0x43; // 'C'
  out[2] = 0x4c; // 'L'
  out[3] = 0x49; // 'I'
  out[4] = ACLI_VERSION;
  out[5] = ACLI_KIND_LANDBLOCK_INSTANCES;
  out[6] = (hasAppearance ? ACLI_FEATURE_APPEARANCE : 0) | (hasAttachments ? ACLI_FEATURE_ATTACHMENTS : 0);
  out[7] = 0;
  dv.setUint32(8, landblock >>> 0, true);
  dv.setUint32(12, instances.length, true);

  let at = ACLI_HEADER_LEN;
  for (const [index, instance] of instances.entries()) {
    dv.setUint32(at, instance.setupId >>> 0, true);
    dv.setFloat32(at + 4, instance.originX, true);
    dv.setFloat32(at + 8, instance.originY, true);
    dv.setFloat32(at + 12, instance.originZ, true);
    dv.setFloat32(at + 16, instance.quatW, true);
    dv.setFloat32(at + 20, instance.quatX, true);
    dv.setFloat32(at + 24, instance.quatY, true);
    dv.setFloat32(at + 28, instance.quatZ, true);
    dv.setFloat32(at + 32, instance.scale, true);
    dv.setUint32(at + 36, instance.paletteId >>> 0, true);
    dv.setUint16(at + 40, instance.paletteOffset, true);
    dv.setUint16(at + 42, instance.paletteLength, true);
    dv.setUint8(at + 44, instance.weenieType);
    dv.setUint8(at + 45, instance.flags);
    dv.setUint16(at + 46, instance.textureOverrides.length, true);
    dv.setUint32(at + 48, instance.motionTableId >>> 0, true);
    dv.setUint32(at + 52, instance.physicsScriptId >>> 0, true);
    dv.setUint32(at + 56, (instance.guid ?? 0) >>> 0, true);
    at += ACLI_RECORD_LEN;
    for (const override of instance.textureOverrides) {
      dv.setUint16(at, override.partIndex, true);
      dv.setUint32(at + 2, override.newTextureId >>> 0, true);
      at += ACLI_OVERRIDE_LEN;
    }
    if (hasAppearance) {
      const appearance = appearances[index];
      dv.setUint32(at, appearance?.length ?? 0, true);
      at += 4;
      if (appearance) {
        out.set(appearance, at);
        at += appearance.length;
      }
    }
  }

  return out;
}

/** Legacy pre-physicsScriptId record base (52 bytes). The decoder accepts it so
 * live ACLI files generated before the in-place layout change still load; the
 * encoder always writes the current 56-byte layout. */
/** Layout with `physicsScriptId` but no `guid` (the current published tree). */
export const ACLI_PREVIOUS_RECORD_LEN = 56;
/** Layout before `physicsScriptId`. */
export const ACLI_LEGACY_RECORD_LEN = 52;

/** Decode one record layout. Throws on truncation/trailing bytes. */
function decodeRecords(
  bytes: Uint8Array,
  dv: DataView,
  count: number,
  recordLen: number,
  hasPhysicsScriptId: boolean,
  hasGuid: boolean,
  hasAppearance = false,
  hasAttachments = false,
): LandblockInstance[] {
  const instances: LandblockInstance[] = [];
  let at = ACLI_HEADER_LEN;
  for (let i = 0; i < count; i++) {
    if (at + recordLen > bytes.byteLength) {
      throw new Error(`ACLI: truncated record ${i}`);
    }
    const setupId = dv.getUint32(at, true);
    const originX = dv.getFloat32(at + 4, true);
    const originY = dv.getFloat32(at + 8, true);
    const originZ = dv.getFloat32(at + 12, true);
    const quatW = dv.getFloat32(at + 16, true);
    const quatX = dv.getFloat32(at + 20, true);
    const quatY = dv.getFloat32(at + 24, true);
    const quatZ = dv.getFloat32(at + 28, true);
    const scale = dv.getFloat32(at + 32, true);
    const paletteId = dv.getUint32(at + 36, true);
    const paletteOffset = dv.getUint16(at + 40, true);
    const paletteLength = dv.getUint16(at + 42, true);
    const weenieType = dv.getUint8(at + 44);
    const flags = dv.getUint8(at + 45);
    const textureCount = dv.getUint16(at + 46, true);
    const motionTableId = dv.getUint32(at + 48, true);
    const physicsScriptId = hasPhysicsScriptId ? dv.getUint32(at + 52, true) : 0;
    const guid = hasGuid ? dv.getUint32(at + 56, true) : 0;
    at += recordLen;

    if (at + textureCount * ACLI_OVERRIDE_LEN > bytes.byteLength) {
      throw new Error(`ACLI: truncated texture overrides in record ${i}`);
    }
    const textureOverrides: TextureOverride[] = [];
    for (let t = 0; t < textureCount; t++) {
      textureOverrides.push({
        partIndex: dv.getUint16(at, true),
        newTextureId: dv.getUint32(at + 2, true),
      });
      at += ACLI_OVERRIDE_LEN;
    }

    let appearance: Appearance | undefined;
    if (hasAppearance) {
      if (at + 4 > bytes.length) throw new Error(`ACLI: truncated appearance size in record ${i}`);
      const size = dv.getUint32(at, true);
      at += 4;
      if (size > bytes.length - at) throw new Error(`ACLI: truncated appearance in record ${i}`);
      if (size) appearance = decodeAppearance(bytes.subarray(at, at + size), hasAttachments);
      at += size;
    }

    instances.push({
      setupId,
      originX,
      originY,
      originZ,
      quatW,
      quatX,
      quatY,
      quatZ,
      scale,
      paletteId,
      paletteOffset,
      paletteLength,
      weenieType,
      flags,
      motionTableId,
      physicsScriptId,
      ...(guid !== 0 ? { guid } : {}),
      textureOverrides,
      ...(appearance ? { appearance } : {}),
    });
  }

  if (at !== bytes.byteLength) {
    throw new Error("ACLI: trailing bytes after records");
  }
  return instances;
}

/** Decode an ACLI file, validating every invariant in the format spec. */
export function decodeAcli(bytes: Uint8Array): LandblockInstances {
  if (bytes.byteLength < ACLI_HEADER_LEN) {
    throw new Error(`ACLI: too short (${bytes.byteLength} bytes)`);
  }
  if (!hasAcliMagic(bytes)) {
    throw new Error("ACLI: bad magic");
  }
  if (bytes[4] !== ACLI_VERSION) {
    throw new Error(`ACLI: unsupported version ${bytes[4]}`);
  }
  if (bytes[5] !== ACLI_KIND_LANDBLOCK_INSTANCES) {
    throw new Error(`ACLI: invalid kind ${bytes[5]}`);
  }

  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const features = dv.getUint16(6, true);
  if ((features & ~(ACLI_FEATURE_APPEARANCE | ACLI_FEATURE_ATTACHMENTS)) !== 0) {
    throw new Error("ACLI: reserved must be zero (unknown feature bits)");
  }
  if ((features & ACLI_FEATURE_ATTACHMENTS) && !(features & ACLI_FEATURE_APPEARANCE)) {
    throw new Error("ACLI: attachments feature requires appearance feature");
  }
  const landblock = dv.getUint32(8, true);
  if (landblock > 0xffff) {
    throw new Error("ACLI: landblock out of range");
  }
  const count = dv.getUint32(12, true);

  // Appearance files have an explicit layout signal, so malformed tails must
  // never be retried as another legacy record layout.
  if (features & ACLI_FEATURE_APPEARANCE) {
    return { landblock, instances: decodeRecords(bytes, dv, count, ACLI_RECORD_LEN, true, true, true, Boolean(features & ACLI_FEATURE_ATTACHMENTS)) };
  }
  let instances: LandblockInstance[];
  try {
    instances = decodeRecords(bytes, dv, count, ACLI_RECORD_LEN, true, true);
  } catch (error) {
    // Newest layout first; fall back through the published tree's layouts so a
    // client keeps reading R2 until the tree is regenerated with guids.
    try {
      instances = decodeRecords(bytes, dv, count, ACLI_PREVIOUS_RECORD_LEN, true, false);
    } catch {
      try {
        instances = decodeRecords(bytes, dv, count, ACLI_LEGACY_RECORD_LEN, false, false);
      } catch {
        throw error;
      }
    }
  }

  return { landblock, instances };
}
