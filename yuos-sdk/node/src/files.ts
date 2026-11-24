// Root-brokered access to one explicitly authorized existing App MCP file.

import { createHash } from "node:crypto";
import { Unavailable, yuCallJson, yuCallJsonAsync } from "./transport";

export const MAX_FILE_BYTES = 32 * 1024 * 1024;
const CHUNK_BYTES = 512 * 1024;
const TOKEN_RE = /^[0-9a-f]{32}$/;
const SHA256_RE = /^sha256:[0-9a-f]{64}$/;

export interface FileSnapshot {
  content: Buffer;
  revision: string;
}

export interface FileOptions {
  binary?: string;
  deadlineUnixMs?: number;
  checkCancelled?: () => void;
}

export interface AsyncFileOptions extends FileOptions {
  signal?: AbortSignal;
}

function validatePath(path: string): void {
  if (
    typeof path !== "string" ||
    !path.startsWith("/") ||
    Buffer.byteLength(path, "utf8") > 4096 ||
    path
      .slice(1)
      .split("/")
      .some((part) => part === "" || part === "." || part === "..") ||
    /[\x00-\x1f\x7f*?[\]{}<>|;&$`\\]/.test(path)
  ) {
    throw new Error("brokered file requires an absolute literal file path");
  }
}

function sha256Hex(data: Buffer): string {
  return `sha256:${createHash("sha256").update(data).digest("hex")}`;
}

function callFileBroker(
  path: string,
  commit: boolean,
  fields: Record<string, unknown>,
  options: FileOptions = {},
): Record<string, unknown> {
  if (options.checkCancelled) options.checkCancelled();
  const timeoutMs = options.deadlineUnixMs
    ? Math.max(250, options.deadlineUnixMs - Date.now())
    : 30_000;
  const payload = JSON.stringify({ path, ...fields });
  return yuCallJson(
    "yu __app_file",
    ["__app_file", commit ? "commit" : "transfer"],
    timeoutMs,
    payload,
    options.binary,
  ) as Record<string, unknown>;
}

async function callFileBrokerAsync(
  path: string,
  commit: boolean,
  fields: Record<string, unknown>,
  options: AsyncFileOptions,
): Promise<Record<string, unknown>> {
  options.checkCancelled?.();
  options.signal?.throwIfAborted();
  const timeoutMs = options.deadlineUnixMs === undefined ? 30_000 : options.deadlineUnixMs - Date.now();
  if (timeoutMs <= 0) throw new Unavailable("brokered file deadline exceeded");
  const reply = await yuCallJsonAsync(
    "yu __app_file",
    ["__app_file", commit ? "commit" : "transfer"],
    options.signal,
    JSON.stringify({ path, ...fields }),
    { binary: options.binary, timeoutMs },
  );
  if (typeof reply !== "object" || reply === null || Array.isArray(reply)) {
    throw new Unavailable("invalid brokered file reply");
  }
  return { ...reply };
}

function extractToken(result: Record<string, unknown>, name: string): string {
  const token = result?.[name];
  if (typeof token !== "string" || !TOKEN_RE.test(token)) {
    throw new Error("broker returned an invalid file transfer token");
  }
  return token;
}

function abortTransfer(path: string, token: string, binary?: string): void {
  try {
    callFileBroker(path, false, { action: "abort", id: token }, { binary, deadlineUnixMs: Date.now() + 250 });
  } catch {
    // ignore abort error
  }
}

async function abortTransferAsync(path: string, token: string, binary?: string): Promise<void> {
  try {
    await callFileBrokerAsync(path, false, { action: "abort", id: token }, { binary, deadlineUnixMs: Date.now() + 250 });
  } catch {
    // The failed transfer's session also owns its cleanup.
  }
}

function readMetadata(metadata: Record<string, unknown>): { size: number; digest: string } {
  const { size, sha256: digest } = metadata;
  if (
    typeof size !== "number" || !Number.isInteger(size) || size < 0 || size > MAX_FILE_BYTES
    || typeof digest !== "string" || !SHA256_RE.test(digest)
  ) {
    throw new Error("broker returned invalid file snapshot metadata");
  }
  return { size, digest };
}

function readChunk(part: Record<string, unknown>, offset: number, size: number): Buffer {
  if (part.offset !== offset || typeof part.content_base64 !== "string") {
    throw new Error("broker returned an invalid file chunk");
  }
  const chunk = Buffer.from(part.content_base64, "base64");
  if (chunk.length !== Math.min(CHUNK_BYTES, size - offset)) {
    throw new Error("broker returned a partial file chunk");
  }
  return chunk;
}

function checkedSnapshot(chunks: Buffer[], digest: string, revision: string): FileSnapshot {
  const content = Buffer.concat(chunks);
  if (sha256Hex(content) !== digest) {
    throw new Error("broker returned a file snapshot with the wrong digest");
  }
  return { content, revision };
}

/** Read an existing file in a `brokered-file` MCP invocation. */
export function readFile(path: string, options: FileOptions = {}): FileSnapshot {
  validatePath(path);
  const metadata = callFileBroker(path, false, { action: "read-start" }, options);
  const revision = extractToken(metadata, "revision");
  let complete = false;
  try {
    const { size, digest } = readMetadata(metadata);
    const chunks: Buffer[] = [];
    for (let offset = 0; offset < size; offset += CHUNK_BYTES) {
      const part = callFileBroker(
        path,
        false,
        { action: "read-chunk", id: revision, offset },
        options,
      );
      chunks.push(readChunk(part, offset, size));
    }
    const snapshot = checkedSnapshot(chunks, digest, revision);
    complete = true;
    return snapshot;
  } finally {
    if (!complete) abortTransfer(path, revision, options.binary);
  }
}

/** Read without blocking the MCP event loop. Pass its CallContext as options. */
export async function readFileAsync(path: string, options: AsyncFileOptions = {}): Promise<FileSnapshot> {
  validatePath(path);
  const metadata = await callFileBrokerAsync(path, false, { action: "read-start" }, options);
  const revision = extractToken(metadata, "revision");
  try {
    const { size, digest } = readMetadata(metadata);
    const chunks: Buffer[] = [];
    for (let offset = 0; offset < size; offset += CHUNK_BYTES) {
      const part = await callFileBrokerAsync(path, false, { action: "read-chunk", id: revision, offset }, options);
      chunks.push(readChunk(part, offset, size));
    }
    return checkedSnapshot(chunks, digest, revision);
  } catch (error) {
    await abortTransferAsync(path, revision, options.binary);
    throw error;
  }
}

function replacement(path: string, revision: string, buf: Buffer) {
  validatePath(path);
  if (typeof revision !== "string" || !TOKEN_RE.test(revision)) {
    throw new Error("brokered file requires its opaque read revision");
  }
  if (buf.length > MAX_FILE_BYTES) {
    throw new Error("replacement must be bytes and at most 32 MiB");
  }
  return { action: "write-start", revision, size: buf.length, sha256: sha256Hex(buf) };
}

function checkWriteChunk(result: Record<string, unknown>, end: number): void {
  if (result.offset !== end) throw new Error("broker did not acknowledge the full file chunk");
}

function checkCommit(result: Record<string, unknown>): void {
  if (result.committed !== true || typeof result.changed !== "boolean") {
    throw new Error("broker returned no conclusive file commit result");
  }
}

/** Replace the exact existing file read earlier in this invocation. */
export function replaceFile(
  path: string,
  revision: string,
  content: Buffer | Uint8Array,
  options: FileOptions = {},
): void {
  const buf = Buffer.isBuffer(content) ? content : Buffer.from(content);
  const started = callFileBroker(
    path,
    false,
    replacement(path, revision, buf),
    options,
  );
  const transfer = extractToken(started, "id");
  let committing = false;
  try {
    for (let offset = 0; offset < buf.length; offset += CHUNK_BYTES) {
      const part = buf.subarray(offset, offset + CHUNK_BYTES);
      const accepted = callFileBroker(
        path,
        false,
        {
          action: "write-chunk",
          id: transfer,
          offset,
          content_base64: part.toString("base64"),
        },
        options,
      );
      checkWriteChunk(accepted, offset + part.length);
    }
    committing = true;
    const result = callFileBroker(path, true, { id: transfer }, options);
    checkCommit(result);
  } finally {
    if (!committing) abortTransfer(path, transfer, options.binary);
  }
}

/** Replace without blocking the event loop. An interrupted commit is not safe to retry. */
export async function replaceFileAsync(
  path: string,
  revision: string,
  content: Buffer | Uint8Array,
  options: AsyncFileOptions = {},
): Promise<void> {
  if (content.byteLength > MAX_FILE_BYTES) {
    throw new Error("replacement must be bytes and at most 32 MiB");
  }
  const buf = Buffer.from(content);
  const started = await callFileBrokerAsync(path, false, replacement(path, revision, buf), options);
  const transfer = extractToken(started, "id");
  try {
    for (let offset = 0; offset < buf.length; offset += CHUNK_BYTES) {
      const part = buf.subarray(offset, offset + CHUNK_BYTES);
      const accepted = await callFileBrokerAsync(path, false, {
        action: "write-chunk", id: transfer, offset, content_base64: part.toString("base64"),
      }, options);
      checkWriteChunk(accepted, offset + part.length);
    }
    options.checkCancelled?.();
    options.signal?.throwIfAborted();
  } catch (error) {
    await abortTransferAsync(path, transfer, options.binary);
    throw error;
  }
  checkCommit(await callFileBrokerAsync(path, true, { id: transfer }, options));
}
