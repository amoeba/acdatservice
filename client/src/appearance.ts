import type { ObjDesc } from "@amoeba/libac";

/**
 * Resolved ACE ObjDesc operations plus service-only held attachments.
 *
 * The part/texture/palette operations are the canonical native `ObjDesc` from
 * `@amoeba/libac`; only the attachment traversal is specific to the ACLI
 * service contract. Palette ranges are in blocks of eight; texture IDs are
 * SurfaceTexture (0x05), part IDs are GfxObj (0x01). Operations retain
 * authored/layer order: later changes win.
 */
export interface Appearance extends ObjDesc {
  /** Separate child Setups, never clothing part replacements. */
  attachments?: HeldAttachment[];
}

export interface HeldAttachment {
  setupId: number;
  /** Key in the parent's Setup.holdingLocations (not a part index). */
  parentLocation: number;
  /** Requested item placement; ACE permits Default (0), never Resting, fallback. */
  placementId: number;
  /** Independent item DefaultScale (not multiplied by the wielder's scale). */
  scale: number;
  /** Resolved against the item's own Setup. */
  appearance?: Appearance;
}

export const MAX_ATTACHMENT_DEPTH = 4;
export const MAX_ATTACHMENT_NODES = 32;


const uint = (n: number, max: number): number => {
  if (!Number.isInteger(n) || n < 0 || n > max) throw new Error("Appearance: integer out of range");
  return n;
};

export function validateHeldAttachment(value: HeldAttachment): void {
  uint(value.setupId, 0xffffffff);
  if (value.setupId >>> 24 !== 2) throw new Error("Appearance: attachment requires Setup id");
  if (![1, 2, 3, 8].includes(value.parentLocation)) throw new Error("Appearance: unsupported parent location");
  if (![1, 2, 3, 6].includes(value.placementId)) throw new Error("Appearance: unsupported held placement");
  if (!Number.isFinite(value.scale) || !Number.isFinite(Math.fround(value.scale)) || Math.fround(value.scale) <= 0) {
    throw new Error("Appearance: attachment scale must be positive finite f32");
  }
}

/** Validate saved/catalog appearance inputs using the same bounds as ACLI. */
export function isAppearance(value: unknown): value is Appearance {
  if (!value || typeof value !== "object") return false;
  try { encodeAppearance(value as Appearance); return true; } catch { return false; }
}

/** Compact ObjDesc: u32 base palette; u16 part/texture/palette/attachment counts;
 * ordered part(6), texture(10), palette(8), then sized attachment records(16+N).
 * `attachmentsEnabled=false` retains the original feature-1 reserved field. */
export function encodeAppearance(value: Appearance, attachmentsEnabled = true): Uint8Array {
  const ancestors = new Set<Appearance>();
  let nodes = 0;
  const encode = (value: Appearance, depth: number): Uint8Array => {
    if (ancestors.has(value)) throw new Error("Appearance: attachment cycle");
    ancestors.add(value);
    if (!Array.isArray(value.parts) || !Array.isArray(value.textures) || !Array.isArray(value.palettes) ||
        (value.attachments !== undefined && !Array.isArray(value.attachments))) throw new Error("Appearance: expected operation arrays");
    const parts = uint(value.parts.length, 0xffff);
    const textures = uint(value.textures.length, 0xffff);
    const palettes = uint(value.palettes.length, 0xffff);
    const attachments = value.attachments ?? [];
    if (attachments.length && !attachmentsEnabled) throw new Error("Appearance: attachments feature required");
    if (attachments.length && depth >= MAX_ATTACHMENT_DEPTH) throw new Error("Appearance: attachment depth exceeded");
    nodes += attachments.length;
    if (nodes > MAX_ATTACHMENT_NODES) throw new Error("Appearance: too many attachments");
    const children = Array.from(attachments, a => {
      validateHeldAttachment(a);
      return a.appearance !== undefined ? encode(a.appearance, depth + 1) : undefined;
    });
    const bytes = new Uint8Array(12 + parts * 6 + textures * 10 + palettes * 8 + children.reduce((sum, child) => sum + 16 + (child?.length ?? 0), 0));
    const dv = new DataView(bytes.buffer);
    dv.setUint32(0, uint(value.paletteBaseId ?? 0, 0xffffffff), true);
    dv.setUint16(4, parts, true);
    dv.setUint16(6, textures, true);
    dv.setUint16(8, palettes, true);
    dv.setUint16(10, attachments.length, true);
    let at = 12;
    for (const p of value.parts) {
      dv.setUint16(at, uint(p.partIndex, 0xffff), true);
      dv.setUint32(at + 2, uint(p.gfxObjId, 0xffffffff), true);
      at += 6;
    }
    for (const t of value.textures) {
      dv.setUint16(at, uint(t.partIndex, 0xffff), true);
      dv.setUint32(at + 2, uint(t.oldTextureId, 0xffffffff), true);
      dv.setUint32(at + 6, uint(t.newTextureId, 0xffffffff), true);
      at += 10;
    }
    for (const p of value.palettes) {
      dv.setUint32(at, uint(p.paletteId, 0xffffffff), true);
      dv.setUint16(at + 4, uint(p.offset, 0xffff), true);
      dv.setUint16(at + 6, uint(p.length, 0xffff), true);
      at += 8;
    }
    attachments.forEach((a, i) => {
      const child = children[i];
      dv.setUint32(at, a.setupId, true);
      dv.setUint16(at + 4, a.parentLocation, true);
      dv.setUint16(at + 6, a.placementId, true);
      dv.setFloat32(at + 8, a.scale, true);
      dv.setUint32(at + 12, child?.length ?? 0, true);
      at += 16;
      if (child) { bytes.set(child, at); at += child.length; }
    });
    ancestors.delete(value);
    return bytes;
  };
  return encode(value, 0);
}

export function decodeAppearance(bytes: Uint8Array, attachmentsEnabled = true): Appearance {
  let nodes = 0;
  const decode = (bytes: Uint8Array, depth: number): Appearance => {
    if (bytes.length < 12) throw new Error("Appearance: truncated header");
    const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const paletteBaseId = dv.getUint32(0, true);
    const parts = dv.getUint16(4, true), textures = dv.getUint16(6, true), palettes = dv.getUint16(8, true);
    const attachments = dv.getUint16(10, true);
    if (!attachmentsEnabled && attachments !== 0) throw new Error("Appearance: reserved must be zero");
    if (attachments && depth >= MAX_ATTACHMENT_DEPTH) throw new Error("Appearance: attachment depth exceeded");
    nodes += attachments;
    if (nodes > MAX_ATTACHMENT_NODES) throw new Error("Appearance: too many attachments");
    if (12 + parts * 6 + textures * 10 + palettes * 8 > bytes.length) throw new Error("Appearance: invalid operation length");
    const value: Appearance = { ...(paletteBaseId ? { paletteBaseId } : {}), parts: [], textures: [], palettes: [] };
    let at = 12;
    for (let i = 0; i < parts; i++, at += 6) value.parts.push({ partIndex: dv.getUint16(at, true), gfxObjId: dv.getUint32(at + 2, true) });
    for (let i = 0; i < textures; i++, at += 10) value.textures.push({ partIndex: dv.getUint16(at, true), oldTextureId: dv.getUint32(at + 2, true), newTextureId: dv.getUint32(at + 6, true) });
    for (let i = 0; i < palettes; i++, at += 8) value.palettes.push({ paletteId: dv.getUint32(at, true), offset: dv.getUint16(at + 4, true), length: dv.getUint16(at + 6, true) });
    if (attachments) value.attachments = [];
    for (let i = 0; i < attachments; i++) {
      if (bytes.length - at < 16) throw new Error("Appearance: truncated attachment");
      const attachment: HeldAttachment = { setupId: dv.getUint32(at, true), parentLocation: dv.getUint16(at + 4, true), placementId: dv.getUint16(at + 6, true), scale: dv.getFloat32(at + 8, true) };
      validateHeldAttachment(attachment);
      const size = dv.getUint32(at + 12, true);
      at += 16;
      if (size > bytes.length - at) throw new Error("Appearance: truncated child appearance");
      if (size) attachment.appearance = decode(bytes.subarray(at, at + size), depth + 1);
      at += size;
      value.attachments!.push(attachment);
    }
    if (at !== bytes.length) throw new Error("Appearance: invalid operation length (trailing bytes)");
    return value;
  };
  return decode(bytes, 0);
}
