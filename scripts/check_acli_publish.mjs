#!/usr/bin/env node
/**
 * Guard for publishing the ACLI landblock-instances tree.
 *
 * ACLI layout changes happen in place (the header version stays 1), so an old
 * client cannot reject a new record layout itself. Before mirroring the tree to
 * R2, this script requires:
 *
 *   1. the deployed marker to accept the record base AND actual header features;
 *   2. every local .bin file to use the current 60-byte base and valid tails,
 *      with a filename whose block id matches the file header.
 *
 * decoderFeatureBits is an optional uint16 marker mask (missing/null = 0),
 * matching ac-noclip's parser. Appearance is mask value 1 (bit 0);
 * attachments add mask value 2 (bit 1) and require both bits.
 *
 * Usage:
 *   node scripts/check_acli_publish.mjs \
 *     --src export/world/landblock-instances \
 *     --client-url https://noclip.treestats.net/acli-layout.json
 *
 * Exit status is non-zero when the publish must not proceed.
 */
import fs from "node:fs";
import path from "node:path";

const RECORD_BASE = 60;
const PREVIOUS_RECORD_BASE = 56;
const LEGACY_RECORD_BASE = 52;
const OVERRIDE_LEN = 6;
const HEADER_LEN = 16;
const ACLI_FEATURE_APPEARANCE = 0x0001;
const ACLI_FEATURE_ATTACHMENTS = 0x0002;
const ACLI_KNOWN_FEATURES = 0x0003;
const MAX_ATTACHMENT_DEPTH = 4;
const MAX_ATTACHMENT_NODES = 32;
const MARKER_FORMAT = "ac-noclip/acli-layout";
const MARKER_VERSION = 1;
const MARKER_KIND = 1;

function argValue(name, fallback) {
  const eq = process.argv.find((arg) => arg.startsWith(`--${name}=`));
  if (eq !== undefined) return eq.slice(name.length + 3);
  const index = process.argv.indexOf(`--${name}`);
  if (index !== -1 && process.argv[index + 1] !== undefined) {
    return process.argv[index + 1];
  }
  return fallback;
}

const srcDir = path.resolve(
  argValue("src", process.env.SRC_DIR ?? "export/world/landblock-instances"),
);
const clientUrl = argValue(
  "client-url",
  process.env.CLIENT_LAYOUT_URL ?? "https://noclip.treestats.net/acli-layout.json",
);

function fail(message) {
  console.error(`ERROR: ${message}`);
  process.exit(1);
}

function u16(bytes, at) {
  return bytes[at] | (bytes[at + 1] << 8);
}

function u32(bytes, at) {
  return (
    (bytes[at] |
      (bytes[at + 1] << 8) |
      (bytes[at + 2] << 16) |
      (bytes[at + 3] << 24)) >>>
    0
  );
}

/** Validate a bounded root appearance; the node budget is shared by all siblings. */
function checkAppearance(bytes, enabled, depth = 0, budget = { nodes: 0 }) {
  if (bytes.length < 12) throw new Error("truncated appearance header");
  const count = u16(bytes, 10);
  if (!enabled && count !== 0) throw new Error("appearance reserved field is not zero");
  const expected = 12 + u16(bytes, 4) * 6 + u16(bytes, 6) * 10 + u16(bytes, 8) * 8;
  if (expected > bytes.length || (count === 0 && expected !== bytes.length)) {
    throw new Error(`invalid appearance operation length: expected ${expected}, got ${bytes.length}`);
  }
  if (count !== 0 && depth >= MAX_ATTACHMENT_DEPTH) throw new Error("attachment depth exceeds 4");
  budget.nodes += count;
  if (budget.nodes > MAX_ATTACHMENT_NODES) throw new Error("attachment nodes exceed 32");
  let at = expected;
  if (count * 16 > bytes.length - at) throw new Error("truncated attachments");
  for (let i = 0; i < count; i++) {
    if (bytes.length - at < 16) throw new Error("truncated attachment header");
    if ((u32(bytes, at) >>> 24) !== 0x02) throw new Error("invalid attachment setup id");
    if (![1, 2, 3, 8].includes(u16(bytes, at + 4))) throw new Error("invalid attachment parent location");
    if (![1, 2, 3, 6].includes(u16(bytes, at + 6))) throw new Error("invalid attachment placement id");
    const scale = bytes.readFloatLE(at + 8);
    if (!Number.isFinite(scale) || scale <= 0) throw new Error("invalid attachment scale");
    const size = u32(bytes, at + 12);
    at += 16;
    if (size > bytes.length - at) throw new Error("truncated child appearance");
    if (size !== 0) checkAppearance(bytes.subarray(at, at + size), enabled, depth + 1, budget);
    at += size;
  }
  if (at !== bytes.length) throw new Error("trailing appearance bytes");
}

/** Parse a file strictly with one record base, or throw. */
function parseWith(bytes, recordBase) {
  if (bytes.length < HEADER_LEN) throw new Error("shorter than the ACLI header");
  if (
    bytes[0] !== 0x41 ||
    bytes[1] !== 0x43 ||
    bytes[2] !== 0x4c ||
    bytes[3] !== 0x49
  ) {
    throw new Error("bad ACLI magic");
  }
  if (bytes[4] !== 1) throw new Error(`unsupported ACLI version ${bytes[4]}`);
  if (bytes[5] !== 1) throw new Error(`invalid ACLI kind ${bytes[5]}`);
  const featureBits = u16(bytes, 6);
  if ((featureBits & ~ACLI_KNOWN_FEATURES) !== 0) {
    throw new Error(`unknown ACLI feature bits 0x${featureBits.toString(16)}`);
  }
  if ((featureBits & ACLI_FEATURE_ATTACHMENTS) && !(featureBits & ACLI_FEATURE_APPEARANCE)) {
    throw new Error("attachment feature requires appearance feature");
  }
  if (featureBits !== 0 && recordBase !== RECORD_BASE) {
    throw new Error("appearance features require the 60-byte record base");
  }
  const landblock = u32(bytes, 8);
  if (landblock > 0xffff) throw new Error("ACLI landblock is out of range");
  const count = u32(bytes, 12);
  let at = HEADER_LEN;
  for (let i = 0; i < count; i++) {
    if (at + recordBase > bytes.length) throw new Error(`truncated record ${i}`);
    const textureCount = u16(bytes, at + 46);
    at += recordBase;
    if (at + textureCount * OVERRIDE_LEN > bytes.length) {
      throw new Error(`truncated texture overrides in record ${i}`);
    }
    at += textureCount * OVERRIDE_LEN;
    if (featureBits & ACLI_FEATURE_APPEARANCE) {
      if (at + 4 > bytes.length) throw new Error(`truncated appearance size in record ${i}`);
      const size = u32(bytes, at);
      at += 4;
      if (size > bytes.length - at) throw new Error(`truncated appearance in record ${i}`);
      if (size !== 0) {
        try {
          checkAppearance(bytes.subarray(at, at + size), Boolean(featureBits & ACLI_FEATURE_ATTACHMENTS));
        } catch (error) {
          throw new Error(`record ${i}: ${error.message}`);
        }
      }
      at += size;
    }
  }
  if (at !== bytes.length) {
    throw new Error(`${bytes.length - at} trailing byte(s) after records`);
  }
  return { landblock, count, featureBits };
}

function inspect(bytes) {
  try {
    return { layout: RECORD_BASE, ...parseWith(bytes, RECORD_BASE) };
  } catch (currentError) {
    // Features explicitly identify the layout; malformed tails must not be
    // reinterpreted as an older fixed record base.
    if (bytes.length >= HEADER_LEN && u16(bytes, 6) !== 0) throw currentError;
    for (const layout of [PREVIOUS_RECORD_BASE, LEGACY_RECORD_BASE]) {
      try {
        return { layout, ...parseWith(bytes, layout) };
      } catch {
        // try the next older layout
      }
    }
    throw currentError;
  }
}

function checkTree() {
  let entries;
  try {
    entries = fs
      .readdirSync(srcDir, { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.toLowerCase().endsWith(".bin"))
      .sort((a, b) => a.name.localeCompare(b.name));
  } catch (error) {
    fail(`cannot read source directory ${srcDir}: ${error.message}`);
  }
  if (entries.length === 0) {
    fail(
      `no .bin files in ${srcDir}; refusing to sync --delete from an empty source`,
    );
  }

  let records = 0;
  let featureBits = 0;
  for (const entry of entries) {
    const match = /^0x([0-9a-f]{4})\.bin$/i.exec(entry.name);
    if (!match) fail(`unexpected ACLI filename (expected 0xNNNN.bin): ${entry.name}`);
    const expectedBlock = Number.parseInt(match[1], 16);
    const bytes = fs.readFileSync(path.join(srcDir, entry.name));
    let info;
    try {
      info = inspect(bytes);
    } catch (error) {
      fail(`${entry.name}: ${error.message}`);
    }
    if (info.layout !== RECORD_BASE) {
      fail(
        `${entry.name}: ${info.layout}-byte record layout; ` +
          `regenerate with the ${RECORD_BASE}-byte generator before publishing`,
      );
    }
    if (info.landblock !== expectedBlock) {
      fail(
        `${entry.name}: file contains landblock 0x${info.landblock
          .toString(16)
          .toUpperCase()
          .padStart(4, "0")}`,
      );
    }
    records += info.count;
    featureBits |= info.featureBits;
  }
  console.error(
    `ACLI tree OK: ${entries.length} file(s), ${records} record(s), ` +
      `${RECORD_BASE}-byte records, featureBits=0x${featureBits.toString(16)}`,
  );
  return featureBits;
}

async function checkClient(requiredFeatureBits) {
  const url = new URL(clientUrl);
  url.searchParams.set("_", String(Date.now()));
  console.error(`client marker: ${url.origin}${url.pathname}`);
  let response;
  try {
    response = await fetch(url, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
  } catch (error) {
    fail(
      `cannot fetch deployed client marker: ${error.message}; deploy the compatible client first`,
    );
  }
  if (!response.ok) {
    fail(
      `deployed client marker returned HTTP ${response.status}; deploy the compatible client first`,
    );
  }
  const text = await response.text();
  let marker;
  try {
    marker = JSON.parse(text);
  } catch {
    fail(
      "deployed client marker is not JSON (is the compatible client deployed?); " +
        `first bytes: ${JSON.stringify(text.slice(0, 60))}`,
    );
  }
  if (
    marker?.format !== MARKER_FORMAT ||
    marker.version !== MARKER_VERSION ||
    marker.kind !== MARKER_KIND ||
    !Array.isArray(marker.decoderRecordBaseBytes) ||
    !marker.decoderRecordBaseBytes.includes(RECORD_BASE)
  ) {
    fail(
      `deployed client does not accept ${RECORD_BASE}-byte ACLI records ` +
        `(marker: ${JSON.stringify(marker)}); deploy the compatible client first`,
    );
  }
  const decoderFeatureBits = marker.decoderFeatureBits ?? 0;
  if (!Number.isInteger(decoderFeatureBits) || decoderFeatureBits < 0 || decoderFeatureBits > 0xffff) {
    fail("deployed client decoderFeatureBits must be a uint16");
  }
  if ((decoderFeatureBits & requiredFeatureBits) !== requiredFeatureBits) {
    fail(
      `deployed client does not accept ACLI featureBits=0x${requiredFeatureBits.toString(16)} ` +
      `(decoderFeatureBits=0x${decoderFeatureBits.toString(16)}); deploy the compatible client first`,
    );
  }
  console.error(
    `client marker OK: decoder accepts ${JSON.stringify(
      marker.decoderRecordBaseBytes,
    )}, decoderFeatureBits=0x${decoderFeatureBits.toString(16)}`,
  );
}

const requiredFeatureBits = checkTree();
await checkClient(requiredFeatureBits);
