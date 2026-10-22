#!/usr/bin/env node
/**
 * Guard for publishing the ACLI landblock-instances tree.
 *
 * ACLI layout changes happen in place (the header version stays 1), so an old
 * client cannot reject a new record layout itself. Before mirroring the tree to
 * R2, this script requires:
 *
 *   1. the deployed client marker to advertise the local record base; and
 *   2. every local .bin file to be in the current 56-byte layout, with a
 *      filename whose block id matches the file header.
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
  if (u16(bytes, 6) !== 0) throw new Error("ACLI reserved field is not zero");
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
  }
  if (at !== bytes.length) {
    throw new Error(`${bytes.length - at} trailing byte(s) after records`);
  }
  return { landblock, count };
}

function inspect(bytes) {
  try {
    return { layout: RECORD_BASE, ...parseWith(bytes, RECORD_BASE) };
  } catch (currentError) {
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
  }
  console.error(
    `ACLI tree OK: ${entries.length} file(s), ${records} record(s), ` +
      `${RECORD_BASE}-byte records`,
  );
}

async function checkClient() {
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
  console.error(
    `client marker OK: decoder accepts ${JSON.stringify(
      marker.decoderRecordBaseBytes,
    )}`,
  );
}

checkTree();
await checkClient();
