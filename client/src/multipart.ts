/** Binary-safe encoder and parser for acdatservice multipart/mixed DAT responses. */

const CR = 0x0d;
const LF = 0x0a;
const DOUBLE_CRLF = new Uint8Array([CR, LF, CR, LF]);

export interface MultipartPart {
  /** File ID extracted from Content-ID: <0xXXXXXXXX>. */
  fileId: number;
  contentType: string;
  filename: string;
  contentLocation: string;
  /** Raw payload bytes, returned as an owned copy. */
  body: Uint8Array;
}

interface Delimiter {
  start: number;
  afterLine: number;
  closing: boolean;
}

function asciiBytes(text: string): Uint8Array {
  const bytes = new Uint8Array(text.length);
  for (let i = 0; i < text.length; i++) bytes[i] = text.charCodeAt(i);
  return bytes;
}

function asciiText(bytes: Uint8Array, start: number, end: number): string {
  let value = "";
  for (let i = start; i < end; i++) value += String.fromCharCode(bytes[i]);
  return value;
}

function indexOfBytes(bytes: Uint8Array, needle: Uint8Array, from: number): number {
  const last = bytes.length - needle.length;
  outer: for (let i = Math.max(0, from); i <= last; i++) {
    for (let j = 0; j < needle.length; j++) {
      if (bytes[i + j] !== needle[j]) continue outer;
    }
    return i;
  }
  return -1;
}

/** Extract the boundary parameter from a multipart Content-Type header. */
export function extractBoundary(contentType: string | null | undefined): string | null {
  if (!contentType) return null;
  const match = /(?:^|;)\s*boundary\s*=\s*(?:"([^"]*)"|([^;\s]*))/i.exec(contentType);
  const boundary = match?.[1] ?? match?.[2];
  return boundary || null;
}

function resolveBoundary(contentTypeOrBoundary: string | null | undefined): string {
  if (!contentTypeOrBoundary) throw new Error("multipart: missing boundary");
  const extracted = extractBoundary(contentTypeOrBoundary);
  const boundary = extracted ?? (/^\s*multipart\//i.test(contentTypeOrBoundary) ? "" : contentTypeOrBoundary.trim());
  if (!boundary) throw new Error("multipart: missing boundary");
  if (boundary.length > 70 || !/^[\x21-\x7e]+$/.test(boundary)) {
    throw new Error("multipart: invalid boundary");
  }
  return boundary;
}

function delimiterAt(bytes: Uint8Array, start: number, marker: Uint8Array): Delimiter | undefined {
  if (start !== 0 && !(bytes[start - 2] === CR && bytes[start - 1] === LF)) return undefined;
  if (start + marker.length > bytes.length) return undefined;
  for (let i = 0; i < marker.length; i++) {
    if (bytes[start + i] !== marker[i]) return undefined;
  }
  let cursor = start + marker.length;
  let closing = false;
  if (bytes[cursor] === 0x2d && bytes[cursor + 1] === 0x2d) {
    closing = true;
    cursor += 2;
  }
  while (bytes[cursor] === 0x20 || bytes[cursor] === 0x09) cursor++;
  if (bytes[cursor] !== CR || bytes[cursor + 1] !== LF) return undefined;
  return { start, afterLine: cursor + 2, closing };
}

function findDelimiter(bytes: Uint8Array, marker: Uint8Array, from: number): Delimiter | undefined {
  let at = from;
  while ((at = indexOfBytes(bytes, marker, at)) !== -1) {
    const match = delimiterAt(bytes, at, marker);
    if (match) return match;
    at++;
  }
  return undefined;
}

function parseHeaders(text: string): Map<string, string> {
  const headers = new Map<string, string>();
  for (const line of text.split("\r\n")) {
    const colon = line.indexOf(":");
    if (colon <= 0) throw new Error("multipart: malformed header");
    const name = line.slice(0, colon).trim().toLowerCase();
    if (!/^[!#$%&'*+.^_`|~0-9a-z-]+$/.test(name)) throw new Error("multipart: invalid header name");
    headers.set(name, line.slice(colon + 1).trim());
  }
  return headers;
}

function parseFileId(value: string | undefined): number | undefined {
  if (!value) return undefined;
  const match = /^\s*<0x([0-9a-f]{1,8})>\s*$/i.exec(value);
  return match ? Number.parseInt(match[1], 16) >>> 0 : undefined;
}

function parseFilename(value: string | undefined): string {
  if (!value) return "";
  const match = /(?:^|;)\s*filename\s*=\s*(?:"((?:\\.|[^"])*)"|([^;\s]*))/i.exec(value);
  return (match?.[1] ?? match?.[2] ?? "").replace(/\\(.)/g, "$1");
}

/**
 * Parse an acdatservice multipart response. The second argument accepts either
 * a complete Content-Type value or a bare boundary token for compatibility.
 * Unidentified parts are skipped, as in the previous libac parser.
 */
export function parseMultipartMixed(
  body: Uint8Array,
  contentTypeOrBoundary: string | null | undefined,
): MultipartPart[] {
  if (!(body instanceof Uint8Array)) throw new TypeError("multipart: body must be Uint8Array");
  const boundary = resolveBoundary(contentTypeOrBoundary);
  const marker = asciiBytes(`--${boundary}`);
  const parts: MultipartPart[] = [];
  let delimiter = findDelimiter(body, marker, 0);
  if (!delimiter) throw new Error("multipart: boundary not found");

  while (!delimiter.closing) {
    const headerEnd = indexOfBytes(body, DOUBLE_CRLF, delimiter.afterLine);
    if (headerEnd < 0) throw new Error("multipart: missing header terminator");
    const dataStart = headerEnd + DOUBLE_CRLF.length;
    const next = findDelimiter(body, marker, dataStart);
    if (!next) throw new Error("multipart: unterminated part");

    // The CRLF immediately before the next delimiter belongs to the framing.
    const dataEnd = next.start >= 2 && body[next.start - 2] === CR && body[next.start - 1] === LF
      ? next.start - 2
      : next.start;
    const headers = parseHeaders(asciiText(body, delimiter.afterLine, headerEnd));
    const fileId = parseFileId(headers.get("content-id"));
    if (fileId !== undefined) {
      parts.push({
        fileId,
        contentType: headers.get("content-type") || "application/octet-stream",
        filename: parseFilename(headers.get("content-disposition")),
        contentLocation: headers.get("content-location") || "",
        body: body.slice(dataStart, dataEnd),
      });
    }
    delimiter = next;
  }
  return parts;
}

/** Input to the service's binary multipart encoder. Metadata is ASCII HTTP
 * header text; payload bytes are never decoded or stringified. */
export interface MultipartInputPart {
  fileId: number;
  body: Uint8Array;
  contentType?: string;
  filename?: string;
  contentLocation?: string;
}

export interface EncodedMultipart {
  contentType: string;
  body: Uint8Array;
}

function headerValue(value: string): string {
  if (!/^[\x20-\x7e]*$/.test(value)) throw new Error("multipart: invalid header value");
  return value;
}

function boundaryCollides(parts: readonly MultipartInputPart[], boundary: string): boolean {
  const marker = asciiBytes(`--${boundary}`);
  return parts.some(part => indexOfBytes(part.body, marker, 0) !== -1);
}

/** Encode the same Content-ID/CRLF framing emitted by the Rust service and
 * parser above. An explicit boundary supports reproducible fixtures; otherwise
 * choose a token absent from every binary payload. */
export function encodeMultipartMixed(
  parts: readonly MultipartInputPart[],
  boundary?: string,
): EncodedMultipart {
  for (const part of parts) {
    if (!Number.isInteger(part.fileId) || part.fileId < 0 || part.fileId > 0xffffffff) {
      throw new RangeError("multipart: file id must be uint32");
    }
    if (!(part.body instanceof Uint8Array)) throw new TypeError("multipart: body must be Uint8Array");
  }
  if (boundary === undefined) {
    for (let attempt = 0; attempt < 100; attempt++) {
      const candidate = `acdatservice-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}-${attempt}`;
      if (!boundaryCollides(parts, candidate)) { boundary = candidate; break; }
    }
    if (boundary === undefined) throw new Error("multipart: could not choose boundary");
  }
  if (!/^[A-Za-z0-9_-]{1,70}$/.test(boundary)) throw new Error("multipart: invalid boundary");
  if (boundaryCollides(parts, boundary)) throw new Error("multipart: boundary occurs in payload");
  const chunks: Uint8Array[] = [];
  let size = 0;
  const append = (bytes: Uint8Array): void => { chunks.push(bytes); size += bytes.byteLength; };
  for (const part of parts) {
    const id = `0x${part.fileId.toString(16).toUpperCase().padStart(8, "0")}`;
    const filename = headerValue(part.filename ?? `${id}.bin`).replace(/\\/g, "\\\\").replace(/"/g, '\\"');
    let headers = `--${boundary}\r\nContent-Type: ${headerValue(part.contentType ?? "application/octet-stream")}\r\nContent-ID: <${id}>\r\nContent-Disposition: attachment; filename="${filename}"\r\n`;
    if (part.contentLocation !== undefined) headers += `Content-Location: ${headerValue(part.contentLocation)}\r\n`;
    append(asciiBytes(`${headers}\r\n`));
    append(part.body);
    append(asciiBytes("\r\n"));
  }
  append(asciiBytes(`--${boundary}--\r\n`));
  const body = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) { body.set(chunk, offset); offset += chunk.byteLength; }
  return { contentType: `multipart/mixed; boundary=${boundary}`, body };
}
