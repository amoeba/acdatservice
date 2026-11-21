import assert from "node:assert/strict";
import test from "node:test";

import {
  ACLI_KIND_LANDBLOCK_INSTANCES,
  ACLI_LEGACY_RECORD_LEN,
  ACLI_PREVIOUS_RECORD_LEN,
  ACLI_RECORD_LEN,
  ACLI_VERSION,
} from "../src/acli.js";
import {
  ACLI_LAYOUT_MARKER_FORMAT,
  ACLI_LAYOUT_MARKER_PATH,
  acliLayoutSupportsRecordBase,
  currentAcliLayoutMarker,
  parseAcliLayoutMarker,
} from "../src/acliLayout.js";

test("current marker advertises the 60-byte encoder and every decoder layout", () => {
  const marker = currentAcliLayoutMarker();
  assert.equal(marker.format, ACLI_LAYOUT_MARKER_FORMAT);
  assert.equal(marker.version, ACLI_VERSION);
  assert.equal(marker.kind, ACLI_KIND_LANDBLOCK_INSTANCES);
  assert.equal(marker.recordBaseBytes, ACLI_RECORD_LEN);
  assert.deepEqual(marker.decoderRecordBaseBytes, [
    ACLI_RECORD_LEN,
    ACLI_PREVIOUS_RECORD_LEN,
    ACLI_LEGACY_RECORD_LEN,
  ]);
  assert.equal(acliLayoutSupportsRecordBase(marker), true);
  assert.equal(acliLayoutSupportsRecordBase(marker, ACLI_PREVIOUS_RECORD_LEN), true);
  assert.equal(acliLayoutSupportsRecordBase(marker, ACLI_LEGACY_RECORD_LEN), true);
  assert.equal(acliLayoutSupportsRecordBase(marker, 64), false);
});


test("rejects incompatible or malformed deployed markers", () => {
  const marker = currentAcliLayoutMarker();
  assert.throws(
    () => parseAcliLayoutMarker({ ...marker, format: "something-else" }),
    /unexpected format/,
  );
  assert.throws(
    () => parseAcliLayoutMarker({ ...marker, version: "1" }),
    /version/,
  );
  assert.throws(
    () => parseAcliLayoutMarker({ ...marker, kind: -1 }),
    /kind/,
  );
  assert.throws(
    () => parseAcliLayoutMarker({ ...marker, recordBaseBytes: 0 }),
    /recordBaseBytes/,
  );
  assert.throws(
    () => parseAcliLayoutMarker({ ...marker, decoderRecordBaseBytes: [] }),
    /decoderRecordBaseBytes/,
  );

  const legacyOnly = parseAcliLayoutMarker({
    ...marker,
    recordBaseBytes: ACLI_LEGACY_RECORD_LEN,
    decoderRecordBaseBytes: [ACLI_LEGACY_RECORD_LEN],
  });
  assert.equal(acliLayoutSupportsRecordBase(legacyOnly), false);
});

test("marker advertises appearance tails independently of unchanged record base", () => {
  const marker = currentAcliLayoutMarker();
  assert.equal(marker.decoderFeatureBits, 3);
  assert.equal(acliLayoutSupportsRecordBase(marker, 60, 3), true);
  assert.equal(acliLayoutSupportsRecordBase({ ...marker, decoderFeatureBits: 1 }, 60, 3), false);
  assert.equal(acliLayoutSupportsRecordBase(marker, 60, 1), true);
  const old = { ...marker, decoderFeatureBits: 0 };
  assert.equal(acliLayoutSupportsRecordBase(old, 60, 1), false);
  assert.equal(acliLayoutSupportsRecordBase(old, 60, 0), true);
  const { decoderFeatureBits: _ignored, ...legacy } = marker;
  assert.equal(parseAcliLayoutMarker(legacy).decoderFeatureBits, 0);
  assert.throws(() => parseAcliLayoutMarker({ ...marker, decoderFeatureBits: -1 }), /decoderFeatureBits/);
});

test("marker path is root-relative for the deployed app", () => {
  assert.equal(ACLI_LAYOUT_MARKER_PATH, "/acli-layout.json");
});
