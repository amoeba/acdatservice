/**
 * ACLI layout contract shared by the client build, the marker served with it,
 * and the acdatservice publish guard.
 *
 * ACLI changes layout in place (the header version stays `1`), so there is no
 * version byte an old client can reject. The deployed client therefore
 * publishes this small marker next to the app; before uploading a regenerated
 * world tree, the publish script requires the *live* marker to advertise that
 * the deployed decoder understands the local record base. The marker is
 * generated from the ACLI constants at build time, so it cannot drift from the
 * decoder that shipped with it.
 *
 * This module has no Node dependencies so the browser can use it too.
 */

import {
  ACLI_FEATURE_APPEARANCE,
  ACLI_FEATURE_ATTACHMENTS,
  ACLI_KIND_LANDBLOCK_INSTANCES,
  ACLI_LEGACY_RECORD_LEN,
  ACLI_PREVIOUS_RECORD_LEN,
  ACLI_RECORD_LEN,
  ACLI_VERSION,
} from "./acli.js";

/** Marker format id; not the ACLI file version. */
export const ACLI_LAYOUT_MARKER_FORMAT = "ac-noclip/acli-layout";
/** Root-relative path served by the deployed client. */
export const ACLI_LAYOUT_MARKER_PATH = "/acli-layout.json";

/**
 * What a deployed client can decode. `recordBaseBytes` is the layout its
 * encoder writes; `decoderRecordBaseBytes` is every record base its decoder
 * accepts (newest first).
 */
export interface AcliLayoutMarker {
  format: string;
  version: number;
  kind: number;
  recordBaseBytes: number;
  decoderRecordBaseBytes: number[];
  /** Recognized ACLI header feature bits (appearance tails = bit 0). */
  decoderFeatureBits: number;
}

/** The marker for the ACLI implementation compiled into this client. */
export function currentAcliLayoutMarker(): AcliLayoutMarker {
  return {
    format: ACLI_LAYOUT_MARKER_FORMAT,
    version: ACLI_VERSION,
    kind: ACLI_KIND_LANDBLOCK_INSTANCES,
    recordBaseBytes: ACLI_RECORD_LEN,
    decoderFeatureBits: ACLI_FEATURE_APPEARANCE | ACLI_FEATURE_ATTACHMENTS,
    decoderRecordBaseBytes: [
      ACLI_RECORD_LEN,
      ACLI_PREVIOUS_RECORD_LEN,
      ACLI_LEGACY_RECORD_LEN,
    ],
  };
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const isNonNegativeInteger = (value: unknown): value is number =>
  typeof value === "number" && Number.isInteger(value) && value >= 0;

/** Parse and validate a JSON marker fetched from a deployed client. */
export function parseAcliLayoutMarker(value: unknown): AcliLayoutMarker {
  if (!isRecord(value)) {
    throw new Error("ACLI layout marker: expected a JSON object");
  }
  if (value.format !== ACLI_LAYOUT_MARKER_FORMAT) {
    throw new Error(
      `ACLI layout marker: unexpected format ${JSON.stringify(value.format)}`,
    );
  }
  if (!isNonNegativeInteger(value.version)) {
    throw new Error("ACLI layout marker: version must be a non-negative integer");
  }
  if (!isNonNegativeInteger(value.kind)) {
    throw new Error("ACLI layout marker: kind must be a non-negative integer");
  }
  if (!isNonNegativeInteger(value.recordBaseBytes) || value.recordBaseBytes === 0) {
    throw new Error("ACLI layout marker: recordBaseBytes must be a positive integer");
  }
  const accepted = value.decoderRecordBaseBytes;
  if (
    !Array.isArray(accepted) ||
    accepted.length === 0 ||
    !accepted.every((entry) => isNonNegativeInteger(entry) && entry > 0)
  ) {
    throw new Error(
      "ACLI layout marker: decoderRecordBaseBytes must be a non-empty array of positive integers",
    );
  }
  const decoderFeatureBits = value.decoderFeatureBits ?? 0;
  if (!isNonNegativeInteger(decoderFeatureBits) || decoderFeatureBits > 0xffff) {
    throw new Error("ACLI layout marker: decoderFeatureBits must be a uint16");
  }
  return {
    format: value.format,
    version: value.version,
    kind: value.kind,
    recordBaseBytes: value.recordBaseBytes,
    decoderRecordBaseBytes: [...accepted],
    decoderFeatureBits,
  };
}

/**
 * True when the deployed client understands files with `requiredRecordBase`.
 * Version and kind are part of the check because a future non-in-place ACLI
 * change cannot rely on layout size alone.
 */
export function acliLayoutSupportsRecordBase(
  marker: AcliLayoutMarker,
  requiredRecordBase: number = ACLI_RECORD_LEN,
  requiredFeatureBits = 0,
): boolean {
  return (
    marker.version === ACLI_VERSION &&
    marker.kind === ACLI_KIND_LANDBLOCK_INSTANCES &&
    marker.decoderRecordBaseBytes.includes(requiredRecordBase) &&
    (marker.decoderFeatureBits & requiredFeatureBits) === requiredFeatureBits
  );
}
