/**
 * Archive names and the historical ID-based route used by the service client.
 *
 * The service URL names the DAT explicitly: `/dats/{archive}/files/{id}`.
 * Callers that know the archive (from a URL, a typed reader, or decoded
 * metadata) should pass it through rather than infer it from the file id.
 * Native AC id spaces overlap: the same 32-bit value can be a cell EnvCell,
 * a WeenieDefaults row, or another family depending on which DAT authored it.
 * There is therefore no universally correct `id -> archive` function.
 */

/** Archive segment accepted by the service `/dats/{archive}/...` routes. */
export type DatName = "portal" | "cell" | "highres" | "local_english";

/**
 * Resource kind a caller already established from context or a typed parser.
 * This is a routing hint only; it is not a native file-type parser.
 */
export type DatResourceKind =
  | "landblock"
  | "landblockInfo"
  | "envcell"
  | "gfxObj"
  | "setup"
  | "animation"
  | "palette"
  | "surfaceTexture"
  | "texture"
  | "surface"
  | "motionTable"
  | "environment"
  | "renderTexture"
  | "renderMaterial"
  | "particleEmitter"
  | "physicsScript"
  | "physicsScriptTable"
  | "localEnglish";

/** Explicit routing information. `archive` always wins. */
export interface DatRouteHint {
  /** Archive named by the caller or service URL. */
  archive?: DatName;
  /** Resource kind the caller already knows. */
  kind?: DatResourceKind;
}

/** Archive object names used by the service's DAT and export tooling. */
export const DAT_OBJECT_KEYS: Readonly<Record<DatName, string>> = Object.freeze({
  portal: "client_portal.dat",
  cell: "client_cell_1.dat",
  highres: "client_highres.dat",
  local_english: "client_local_english.dat",
});

const CELL_KINDS: ReadonlySet<DatResourceKind> = new Set(["landblock", "landblockInfo", "envcell"]);

/**
 * Parse a service archive parameter, accepting the same aliases as the Rust
 * `parse_dat_param`: `portal`, `cell`, `highres`, `local_english`, their
 * `client_*.dat` object keys, and `local-english`.
 */
export function parseDatName(value: string): DatName {
  switch (value.trim().toLowerCase()) {
    case "portal":
    case "client_portal.dat":
      return "portal";
    case "cell":
    case "client_cell.dat":
    case "client_cell_1.dat":
      return "cell";
    case "highres":
    case "client_highres.dat":
      return "highres";
    case "local_english":
    case "local-english":
    case "client_local_english.dat":
      return "local_english";
    default:
      throw new Error(
        `Invalid DAT name: ${value}. Expected portal, cell, highres, or local_english.`,
      );
  }
}

/** DAT object name for an archive, e.g. `portal` -> `client_portal.dat`. */
export function datObjectKey(dat: DatName): string {
  return DAT_OBJECT_KEYS[dat];
}

/**
 * Resolve the archive for a resource.
 *
 * Precedence: explicit `archive`, then an explicit `kind`, then the historical
 * ID-based fallback. Prefer passing `archive` (or the `kind` plus a known
 * context) whenever the caller has it.
 *
 * The fallback is retained for untyped legacy callers and is not a statement
 * about native AC identity. In particular it cannot distinguish overlapping
 * spaces: `datFor(0x3300067a)` reports `cell` even though that value can be a
 * portal PhysicsScript. Pass `{ archive: "portal" }` for such resources.
 */
export function datFor(id: number, hint: DatRouteHint = {}): DatName {
  if (hint.archive) return hint.archive;
  if (hint.kind === "localEnglish") return "local_english";
  if (hint.kind) return CELL_KINDS.has(hint.kind) ? "cell" : "portal";

  // --- Historical fallback (unverified id guess; do not promote as truth) ---
  const suffix = id & 0xffff;
  // CellLandblock (…FFFF) and LandblockInfo (…FFFE) live in the cell dat.
  if (suffix === 0xffff || suffix === 0xfffe) return "cell";
  // Graphics families (GfxObj 0x01, Setup 0x02, …) live in the portal dat.
  const hi = (id >>> 24) & 0xff;
  if (hi >= 0x01 && hi <= 0x1f) return "portal";
  // Indoor EnvCells use a landblock in the high word and start at cell 0x100.
  // AcArchives routes them explicitly because low landblock IDs can overlap
  // portal resource ranges.
  if (suffix >= 0x0100) return "cell";
  // EnvCell (cell) and local_english are not routed yet. Highres texture
  // overrides share portal 0x06 ids, so AcArchives.readTexture probes highres
  // explicitly after a portal miss rather than changing the id-based route.
  return "portal";
}
