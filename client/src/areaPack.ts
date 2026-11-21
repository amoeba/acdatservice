import type { DatName } from "./datRouting.js";

const HEADER_LEN = 16;
const VERSION = 1;
const MAX_JSON_BYTES = 4 * 1024 * 1024;
const MAX_ENTRIES = 100_000;
const MAX_PACK_BYTES = 256 * 1024 * 1024;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

export interface AreaPackMetadata {
  areaId: string;
  blocks: number[];
  acliRevision: string;
}

type IndexEntry = { key: string; offset: number; length: number | null };

export function areaFileKey(id: number, dat: DatName): string {
  return `${dat}:${(id >>> 0).toString(16).padStart(8, "0")}`;
}

export function areaInstanceKey(block: number, revision: string): string {
  return `instances:${revision}:${(block & 0xffff).toString(16).padStart(4, "0")}`;
}

function validRevision(value: unknown): value is string {
  return typeof value === "string" && /^[A-Za-z0-9_-]{1,64}$/.test(value);
}

function validateKey(key: unknown, revision: string): asserts key is string {
  if (typeof key !== "string") throw new Error("Area pack: key must be a string");
  if (/^(portal|cell|highres|local_english):[0-9a-f]{8}$/.test(key)) return;
  const match = /^instances:([A-Za-z0-9_-]{1,64}):[0-9a-f]{4}$/.exec(key);
  if (!match || match[1] !== revision) throw new Error(`Area pack: invalid key ${key}`);
}

function normalizeMetadata(value: unknown): AreaPackMetadata {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Area pack: invalid metadata");
  const m = value as Record<string, unknown>;
  if (typeof m.areaId !== "string" || !/^[a-z0-9][a-z0-9_-]{0,63}$/.test(m.areaId)) {
    throw new Error("Area pack: invalid areaId");
  }
  if (!validRevision(m.acliRevision)) throw new Error("Area pack: invalid ACLI revision");
  if (!Array.isArray(m.blocks)) throw new Error("Area pack: blocks must be an array");
  const blocks: number[] = [];
  for (const block of m.blocks) {
    if (!Number.isInteger(block) || (block as number) < 0 || (block as number) > 0xffff) {
      throw new Error("Area pack: block out of bounds");
    }
    blocks.push(block as number);
  }
  blocks.sort((a, b) => a - b);
  for (let i = 1; i < blocks.length; i++) {
    if (blocks[i] === blocks[i - 1]) throw new Error("Area pack: duplicate block");
  }
  return { areaId: m.areaId, blocks, acliRevision: m.acliRevision };
}

export function encodeAreaPack(
  metadata: AreaPackMetadata,
  entries: Iterable<readonly [string, Uint8Array | undefined]>,
): Uint8Array {
  const normalized = normalizeMetadata(metadata);
  const sorted = [...entries].sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0);
  if (sorted.length > MAX_ENTRIES) throw new Error("Area pack: too many entries");
  const seen = new Set<string>();
  let payloadLength = 0;
  const index: IndexEntry[] = [];
  for (const [key, data] of sorted) {
    validateKey(key, normalized.acliRevision);
    if (seen.has(key)) throw new Error(`Area pack: duplicate key ${key}`);
    seen.add(key);
    if (data !== undefined && !(data instanceof Uint8Array)) throw new Error("Area pack: entry data must be Uint8Array or undefined");
    const length = data?.byteLength ?? null;
    index.push({ key, offset: data === undefined ? 0 : payloadLength, length });
    if (data) payloadLength += data.byteLength;
  }
  const json = encoder.encode(JSON.stringify({ metadata: normalized, index }));
  if (json.byteLength > MAX_JSON_BYTES) throw new Error("Area pack: JSON exceeds limit");
  const total = HEADER_LEN + json.byteLength + payloadLength;
  if (total > MAX_PACK_BYTES) throw new Error("Area pack: pack exceeds limit");
  const out = new Uint8Array(total);
  out.set([0x41, 0x43, 0x41, 0x50, VERSION], 0);
  const view = new DataView(out.buffer);
  view.setUint32(8, json.byteLength, true);
  view.setUint32(12, sorted.length, true);
  out.set(json, HEADER_LEN);
  let at = HEADER_LEN + json.byteLength;
  for (const [, data] of sorted) {
    if (data !== undefined) {
      out.set(data, at);
      at += data.byteLength;
    }
  }
  return out;
}

export function decodeAreaPack(bytes: Uint8Array): { metadata: AreaPackMetadata; entries: Map<string, Uint8Array | undefined> } {
  if (!(bytes instanceof Uint8Array) || bytes.byteLength < HEADER_LEN) throw new Error("Area pack: truncated header");
  if (bytes.byteLength > MAX_PACK_BYTES) throw new Error("Area pack: pack exceeds limit");
  if (bytes[0] !== 0x41 || bytes[1] !== 0x43 || bytes[2] !== 0x41 || bytes[3] !== 0x50) throw new Error("Area pack: bad magic");
  if (bytes[4] !== VERSION) throw new Error(`Area pack: unsupported version ${bytes[4]}`);
  if (bytes[5] !== 0 || bytes[6] !== 0 || bytes[7] !== 0) throw new Error("Area pack: reserved bytes must be zero");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const jsonLength = view.getUint32(8, true);
  const count = view.getUint32(12, true);
  if (jsonLength > MAX_JSON_BYTES) throw new Error("Area pack: JSON exceeds limit");
  if (count > MAX_ENTRIES) throw new Error("Area pack: too many entries");
  const payloadAt = HEADER_LEN + jsonLength;
  if (payloadAt > bytes.byteLength) throw new Error("Area pack: truncated JSON");
  let parsed: unknown;
  try {
    parsed = JSON.parse(decoder.decode(bytes.subarray(HEADER_LEN, payloadAt)));
  } catch {
    throw new Error("Area pack: invalid UTF-8 or JSON");
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("Area pack: invalid document");
  const doc = parsed as Record<string, unknown>;
  const metadata = normalizeMetadata(doc.metadata);
  if (!Array.isArray(doc.index) || doc.index.length !== count) throw new Error("Area pack: entry count mismatch");
  const entries = new Map<string, Uint8Array | undefined>();
  const ranges: Array<[number, number]> = [];
  const payloadLength = bytes.byteLength - payloadAt;
  for (const raw of doc.index) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) throw new Error("Area pack: invalid index entry");
    const e = raw as Record<string, unknown>;
    validateKey(e.key, metadata.acliRevision);
    if (entries.has(e.key)) throw new Error(`Area pack: duplicate key ${e.key}`);
    if (!Number.isInteger(e.offset) || (e.offset as number) < 0) throw new Error("Area pack: invalid offset");
    if (e.length === null) {
      if (e.offset !== 0) throw new Error("Area pack: missing entry offset must be zero");
      entries.set(e.key, undefined);
      continue;
    }
    if (!Number.isInteger(e.length) || (e.length as number) < 0) throw new Error("Area pack: invalid length");
    const offset = e.offset as number;
    const length = e.length as number;
    if (offset > payloadLength || length > payloadLength - offset) throw new Error("Area pack: entry out of bounds");
    if (length > 0) ranges.push([offset, offset + length]);
    entries.set(e.key, bytes.subarray(payloadAt + offset, payloadAt + offset + length));
  }
  ranges.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  let cursor = 0;
  for (const [start, end] of ranges) {
    if (start !== cursor) throw new Error("Area pack: payload ranges do not tile exactly");
    cursor = end;
  }
  if (cursor !== payloadLength) throw new Error("Area pack: trailing payload bytes");
  return { metadata, entries };
}
