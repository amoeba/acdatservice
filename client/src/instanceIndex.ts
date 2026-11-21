/**
 * ACVC: the compact instance index the Views panel browses.
 *
 * 66,667 rows of `{name, blockId, setupId, weenieType, motionTableId, ordinal}`
 * are ~1 MB as a typed-array file and ~160 KB brotli, against 14.6 MB for the
 * full placement catalog and 6.4 MB for the same rows as JSON
 * (research/catalog-delivery-options.md, #75). Decoding is a header read plus
 * typed-array views — no per-row objects, no key names — so it costs
 * microseconds and allocates nothing per row.
 *
 * Layout (little-endian, every section 4-byte aligned):
 *
 *   "ACVC" u8 version=2 u8 kind=2 u16 reserved
 *   u32 stringCount | u32 stringBytes
 *   u16 length[stringCount] | utf8 bytes[stringBytes]
 *   u32 rowCount
 *   u16 nameId[] | u16 blockId[] | u32 setupId[] | u8 weenieType[]
 *   u32 motionTableId[] | u16 ordinal[] | u32 guid[]
 *
 * Version 1 had no `guid` column and is still decoded (guid 0) so a client can
 * read the previous artifact until it is regenerated.
 *
 * `ordinal` is the record's 0-based position among the block's instances of that
 * Setup in ACLI (GUID) order — the same value `?instance=block:setup:nth` uses,
 * so clicking a row produces a canonical link (#75).
 */
export const ACVC_MAGIC = "ACVC";
export const ACVC_VERSION = 2;
/** Version 1: same columns without `guid`. */
export const ACVC_LEGACY_VERSION = 1;
export const ACVC_KIND_INSTANCE_INDEX = 2;
const HEADER_LEN = 16;

export interface InstanceIndexRow {
  name: string;
  blockId: number;
  setupId: number;
  weenieType: number;
  motionTableId: number;
  ordinal: number;
  /** ACE weenie GUID; 0 when the row predates the 60-byte ACLI layout. */
  guid: number;
}

/** A decoded index: column arrays plus a dictionary lookup for names. */
export interface InstanceIndex {
  rowCount: number;
  names: readonly string[];
  nameId: Uint16Array;
  blockId: Uint16Array;
  setupId: Uint32Array;
  weenieType: Uint8Array;
  motionTableId: Uint32Array;
  ordinal: Uint16Array;
  /** ACE weenie GUID per row (0 for version-1 files). */
  guid: Uint32Array;
  /** `names[nameId[i]]` */
  name(i: number): string;
}

const align4 = (n: number) => (n + 3) & ~3;

/** Encode rows into an ACVC instance index. Dictionary order is input order. */
export function encodeInstanceIndex(rows: readonly InstanceIndexRow[]): Uint8Array {
  const encoder = new TextEncoder();
  const dictionary: string[] = [];
  const ids = new Map<string, number>();
  const nameId = new Uint16Array(rows.length);
  for (let i = 0; i < rows.length; i++) {
    const { name } = rows[i];
    let id = ids.get(name);
    if (id === undefined) {
      if (dictionary.length >= 0x10000) {
        throw new RangeError("ACVC: more than 65,536 distinct names");
      }
      id = dictionary.length;
      ids.set(name, id);
      dictionary.push(name);
    }
    nameId[i] = id;
  }
  const encodedNames = dictionary.map((name) => encoder.encode(name));
  for (const bytes of encodedNames) {
    if (bytes.length > 0xffff) throw new RangeError("ACVC: a name exceeds 65535 bytes");
  }
  const dictionaryBytes = encodedNames.reduce((sum, bytes) => sum + bytes.length, 0);

  // Lay out every section on a 4-byte boundary so each typed-array view is
  // aligned (and therefore constructible directly over the file's buffer).
  const sections: Record<string, number> = {};
  let total = HEADER_LEN;
  const reserve = (name: string, bytes: number) => {
    total = align4(total);
    sections[name] = total;
    total += bytes;
  };
  reserve("lengths", dictionary.length * 2);
  reserve("dictionary", dictionaryBytes);
  reserve("count", 4);
  reserve("nameId", rows.length * 2);
  reserve("blockId", rows.length * 2);
  reserve("setupId", rows.length * 4);
  reserve("weenieType", rows.length);
  reserve("motionTableId", rows.length * 4);
  reserve("ordinal", rows.length * 2);
  reserve("guid", rows.length * 4);

  const out = new Uint8Array(align4(total));
  const view = new DataView(out.buffer);
  out.set(encoder.encode(ACVC_MAGIC), 0);
  view.setUint8(4, ACVC_VERSION);
  view.setUint8(5, ACVC_KIND_INSTANCE_INDEX);
  view.setUint32(8, dictionary.length, true);
  view.setUint32(12, dictionaryBytes, true);

  const lengths = new Uint16Array(out.buffer, sections.lengths, dictionary.length);
  let cursor = sections.dictionary;
  encodedNames.forEach((bytes, i) => {
    lengths[i] = bytes.length;
    out.set(bytes, cursor);
    cursor += bytes.length;
  });

  view.setUint32(sections.count, rows.length, true);
  new Uint16Array(out.buffer, sections.nameId, rows.length).set(nameId);
  new Uint16Array(out.buffer, sections.blockId, rows.length).set(
    Uint16Array.from(rows, (row) => row.blockId & 0xffff),
  );
  new Uint32Array(out.buffer, sections.setupId, rows.length).set(
    Uint32Array.from(rows, (row) => row.setupId >>> 0),
  );
  new Uint8Array(out.buffer, sections.weenieType, rows.length).set(
    Uint8Array.from(rows, (row) => row.weenieType & 0xff),
  );
  new Uint32Array(out.buffer, sections.motionTableId, rows.length).set(
    Uint32Array.from(rows, (row) => row.motionTableId >>> 0),
  );
  new Uint16Array(out.buffer, sections.ordinal, rows.length).set(
    Uint16Array.from(rows, (row) => row.ordinal),
  );
  new Uint32Array(out.buffer, sections.guid, rows.length).set(
    Uint32Array.from(rows, (row) => row.guid >>> 0),
  );
  return out;
}

function failure(message: string): never {
  throw new Error(`ACVC: ${message}`);
}

/** Decode an ACVC instance index. Throws on a malformed or truncated file. */
export function decodeInstanceIndex(bytes: Uint8Array): InstanceIndex {
  if (bytes.byteLength < HEADER_LEN + 4) failure("truncated header");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const magic = String.fromCharCode(...bytes.subarray(0, 4));
  if (magic !== ACVC_MAGIC) failure(`bad magic "${magic}"`);
  const version = view.getUint8(4);
  if (version !== ACVC_VERSION && version !== ACVC_LEGACY_VERSION) {
    failure(`unsupported version ${version}`);
  }
  if (view.getUint8(5) !== ACVC_KIND_INSTANCE_INDEX) {
    failure(`not an instance index (kind ${view.getUint8(5)})`);
  }
  const stringCount = view.getUint32(8, true);
  const stringBytes = view.getUint32(12, true);

  let offset = HEADER_LEN;
  const need = (n: number, what: string) => {
    if (offset + n > bytes.byteLength) failure(`truncated ${what}`);
  };

  need(stringCount * 2, "string lengths");
  const lengths = new Uint16Array(bytes.buffer, bytes.byteOffset + offset, stringCount);
  offset = align4(offset + stringCount * 2);

  need(stringBytes, "string table");
  const table = bytes.subarray(offset, offset + stringBytes);
  offset = align4(offset + stringBytes);
  const decoder = new TextDecoder();
  const names: string[] = [];
  let cursor = 0;
  for (let i = 0; i < stringCount; i++) {
    const length = lengths[i];
    if (cursor + length > table.byteLength) failure("name runs past the string table");
    names.push(decoder.decode(table.subarray(cursor, cursor + length)));
    cursor += length;
  }

  need(4, "row count");
  const rowCount = view.getUint32(offset, true);
  offset = align4(offset + 4);

  const column = <T extends Uint16Array | Uint32Array | Uint8Array>(
    Ctor: new (buffer: ArrayBufferLike, byteOffset: number, length: number) => T,
    width: number,
    what: string,
  ): T => {
    need(rowCount * width, what);
    const array = new Ctor(bytes.buffer, bytes.byteOffset + offset, rowCount);
    offset = align4(offset + rowCount * width);
    return array;
  };

  const nameId = column(Uint16Array, 2, "name ids");
  const blockId = column(Uint16Array, 2, "block ids");
  const setupId = column(Uint32Array, 4, "setup ids");
  const weenieType = column(Uint8Array, 1, "weenie types");
  const motionTableId = column(Uint32Array, 4, "motion tables");
  const ordinal = column(Uint16Array, 2, "ordinals");
  const guid =
    version >= 2
      ? column(Uint32Array, 4, "guids")
      : new Uint32Array(rowCount);

  for (let i = 0; i < rowCount; i++) {
    if (nameId[i] >= stringCount) failure(`row ${i} has no name`);
  }

  return {
    rowCount,
    names,
    nameId,
    blockId,
    setupId,
    weenieType,
    motionTableId,
    ordinal,
    guid,
    name(i: number) {
      return names[nameId[i]] ?? "";
    },
  };
}

/** The `manifest.json` a client reads to find the current, content-hashed file. */
export interface InstanceIndexManifest {
  format: "ACVC";
  version: number;
  rows: number;
  bytes: number;
  /** `instance-index-<hash>.bin`, immutable-cached. */
  file: string;
  /** First 16 hex digits of the file's SHA-256. */
  hash: string;
  generatedAt: string;
  /** Size of the `.gz` sibling the client fetches and inflates. */
  gzipBytes?: number;
}

export function isInstanceIndexManifest(value: unknown): value is InstanceIndexManifest {
  if (typeof value !== "object" || value === null) return false;
  const manifest = value as Record<string, unknown>;
  return (
    manifest.format === "ACVC" &&
    (manifest.version === ACVC_VERSION || manifest.version === ACVC_LEGACY_VERSION) &&
    typeof manifest.rows === "number" &&
    Number.isInteger(manifest.rows) &&
    manifest.rows >= 0 &&
    typeof manifest.bytes === "number" &&
    Number.isInteger(manifest.bytes) &&
    manifest.bytes > 0 &&
    typeof manifest.file === "string" &&
    /^instance-index-[0-9a-f]{16}\.bin$/.test(manifest.file) &&
    typeof manifest.hash === "string" &&
    manifest.hash.length === 16 &&
    typeof manifest.generatedAt === "string"
  );
}
