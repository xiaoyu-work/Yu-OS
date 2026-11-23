// Root-brokered access to one explicitly authorized existing App MCP file.

import { createHash } from "node:crypto";
import { yuCallJson } from "./transport";

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
  ) as Record<string, unknown>;
}

function extractToken(result: Record<string, unknown>, name: string): string {
  const token = result?.[name];
  if (typeof token !== "string" || !TOKEN_RE.test(token)) {
    throw new Error("broker returned an invalid file transfer token");
  }
  return token;
}

function abortTransfer(path: string, token: string): void {
  try {
    callFileBroker(path, false, { action: "abort", id: token }, { deadlineUnixMs: Date.now() + 250 });
  } catch {
    // ignore abort error
  }
}

/** Read an existing file in a `brokered-file` MCP invocation. */
export function readFile(path: string, options: FileOptions = {}): FileSnapshot {
  validatePath(path);
  const metadata = callFileBroker(path, false, { action: "read-start" }, options);
  const revision = extractToken(metadata, "revision");
  let complete = false;
  try {
    const size = metadata.size;
    const digest = metadata.sha256;
    if (
      typeof size !== "number" ||
      !Number.isInteger(size) ||
      size < 0 ||
      size > MAX_FILE_BYTES ||
      typeof digest !== "string" ||
      !SHA256_RE.test(digest)
    ) {
      throw new Error("broker returned invalid file snapshot metadata");
    }
    const chunks: Buffer[] = [];
    for (let offset = 0; offset < size; offset += CHUNK_BYTES) {
      const part = callFileBroker(
        path,
        false,
        { action: "read-chunk", id: revision, offset },
        options,
      );
      const count = Math.min(CHUNK_BYTES, size - offset);
      const encoded = part.content_base64;
      if (part.offset !== offset || typeof encoded !== "string") {
        throw new Error("broker returned an invalid file chunk");
      }
      const chunk = Buffer.from(encoded, "base64");
      if (chunk.length !== count) {
        throw new Error("broker returned a partial file chunk");
      }
      chunks.push(chunk);
    }
    const content = Buffer.concat(chunks);
    if (sha256Hex(content) !== digest) {
      throw new Error("broker returned a file snapshot with the wrong digest");
    }
    complete = true;
    return { content, revision };
  } finally {
    if (!complete) abortTransfer(path, revision);
  }
}

/** Replace the exact existing file read earlier in this invocation. */
export function replaceFile(
  path: string,
  revision: string,
  content: Buffer | Uint8Array,
  options: FileOptions = {},
): void {
  validatePath(path);
  if (typeof revision !== "string" || !TOKEN_RE.test(revision)) {
    throw new Error("brokered file requires its opaque read revision");
  }
  const buf = Buffer.isBuffer(content) ? content : Buffer.from(content);
  if (buf.length > MAX_FILE_BYTES) {
    throw new Error("replacement must be bytes and at most 32 MiB");
  }
  const started = callFileBroker(
    path,
    false,
    {
      action: "write-start",
      revision,
      size: buf.length,
      sha256: sha256Hex(buf),
    },
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
      if (accepted.offset !== offset + part.length) {
        throw new Error("broker did not acknowledge the full file chunk");
      }
    }
    committing = true;
    const result = callFileBroker(path, true, { id: transfer }, options);
    if (result.committed !== true || typeof result.changed !== "boolean") {
      throw new Error("broker returned no conclusive file commit result");
    }
  } finally {
    if (!committing) abortTransfer(path, transfer);
  }
}
