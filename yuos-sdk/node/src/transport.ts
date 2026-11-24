// Subprocess transport shared by every Node SDK module.
//
// Like the Python and Rust SDKs, the Node SDK is a thin client over
// wire protocol v1: it shells out to the `yu` binary, which reads
// non-sensitive routing flags from argv and writes a JSON envelope to stdout.
// AI prompt bodies are passed through private temporary files. The
// subprocess model is intentional — identity, audit, and session
// context are inherited from process ancestry (kernel-spawned parent →
// app process → yu child). A pure in-process binding could not prove
// "App X is making this call".

import { execFile, spawn, spawnSync } from "node:child_process";
import { WireDecodeError, decodeWireJson, validateAiChatDelta, validateEnvelope } from "./generated";

/** Default per-call timeout. Bounded so a wedged child never blocks
 * the calling app forever, but long enough for slow providers. */
export const DEFAULT_TIMEOUT_MS = 60_000;

/** Max bytes captured from a child's stdout/stderr (8 MiB). Generous
 * enough for embeddings / base64 artifact paths without unbounded RAM. */
const MAX_BUFFER = 8 * 1024 * 1024;
const WIRE_ARG = "--wire=1";

/**
 * Resolve the `yu` binary.
 *
 * Honors `YUOS_KERNEL_BIN`, falling back to `yu` on `$PATH`.
 */
export function yuBinary(): string {
  return process.env.YUOS_KERNEL_BIN || "yu";
}

/** Base class for every error the transport (or a module built on it)
 * raises. */
export class BridgeError extends Error {}

/** The `yu` binary could not be invoked, timed out, or returned
 * something that was not a JSON envelope. */
export class Unavailable extends BridgeError {}

/** A gate (capability / origin / budget / unknown verb / arg shape)
 * refused the call. `payload` holds the structured error envelope the
 * kernel returned, suitable for forwarding back to the agent. */
export class Denied extends BridgeError {
  readonly payload: Record<string, unknown>;
  constructor(payload: Record<string, unknown>) {
    super(String(payload["error"] ?? "call denied"));
    this.payload = payload;
  }
}

function truncate(value: string, limit = 200): string {
  if (value.length <= limit) return value;
  return value.slice(0, limit) + `... [${value.length - limit} more bytes elided]`;
}

/**
 * Run `yu --wire=1 <args>` synchronously and return the success data.
 *
 * `label` names the logical call (e.g. `"yu ai chat"`) for error
 * messages. Throws {@link Unavailable} for transport problems (binary
 * missing, timeout, malformed protocol). Throws {@link Denied} for a
 * valid kernel error envelope.
 */
export function yuCallJson(
  label: string,
  args: string[],
  timeoutMs: number = DEFAULT_TIMEOUT_MS,
  input?: string,
  binary?: string,
): unknown {
  const bin = binary ?? yuBinary();
  const res = spawnSync(bin, [WIRE_ARG, ...args], {
    encoding: "utf8",
    timeout: timeoutMs,
    maxBuffer: MAX_BUFFER,
    input,
  });

  if (res.error) {
    const code = (res.error as NodeJS.ErrnoException).code;
    if (code === "ETIMEDOUT") {
      throw new Unavailable(`${label} timed out after ${timeoutMs}ms`);
    }
    if (code === "ENOENT") {
      throw new Unavailable(
        `the \`${bin}\` binary is not on PATH; cannot reach the kernel ` +
          `(set YUOS_KERNEL_BIN or install yu)`,
      );
    }
    throw new Unavailable(`could not spawn ${bin}: ${res.error.message}`);
  }

  return decodeResponse(label, res.stdout || "", res.status ?? -1);
}

/** Run a wire-v1 call without blocking the event loop. */
export async function yuCallJsonAsync(
  label: string,
  args: string[],
  signal?: AbortSignal,
  input?: string,
  { binary = yuBinary(), timeoutMs = DEFAULT_TIMEOUT_MS }: { binary?: string; timeoutMs?: number } = {},
): Promise<unknown> {
  signal?.throwIfAborted();
  return new Promise((resolve, reject) => {
    const child = execFile(binary, [WIRE_ARG, ...args], {
      encoding: "utf8",
      timeout: timeoutMs,
      maxBuffer: MAX_BUFFER,
      killSignal: "SIGKILL",
      signal,
    }, (error, stdout) => {
      if (signal?.aborted) {
        reject(signal.reason);
        return;
      }

      const status = error ? error.code : 0;
      if (typeof status !== "number") {
        const message = error?.killed
          ? `${label} timed out after ${timeoutMs}ms`
          : `${label} subprocess failed (${status ?? error?.signal ?? "no exit code"})`;
        reject(new Unavailable(message));
        return;
      }
      try {
        resolve(decodeResponse(label, stdout, status));
      } catch (error) {
        reject(error);
      }
    });
    // The CLI can refuse before draining stdin; its envelope is authoritative.
    child.stdin?.on("error", (error: NodeJS.ErrnoException) => {
      if (error.code !== "EPIPE" && !signal?.aborted) {
        child.kill("SIGKILL");
        reject(new Unavailable(`${label} could not send its request`));
      }
    });
    child.stdin?.end(input);
  });
}

function decodeResponse(label: string, stdout: string, status: number): unknown {
  const text = stdout.trim();
  if (!text) {
    throw new Unavailable(`${label} returned no wire response (exit ${status})`);
  }

  let envelope: unknown;
  try {
    envelope = decodeWireJson(text);
  } catch {
    throw new Unavailable(`${label} returned non-JSON output: ${truncate(text)}`);
  }
  try {
    validateEnvelope(envelope);
  } catch (error) {
    if (error instanceof WireDecodeError) {
      throw new Unavailable(`${label} returned an invalid wire envelope: ${error.message}`);
    }
    throw error;
  }

  if (envelope.ok) {
    if (status !== 0) {
      throw new Unavailable(`${label} returned a success envelope with exit ${status}`);
    }
    return envelope.data;
  }
  if (status === 0) {
    throw new Unavailable(`${label} returned an error envelope with exit 0`);
  }
  throw new Denied(envelope);
}

/** UTF-8 lines with a byte limit even when the producer omits a newline. */
export async function* boundedLines(
  chunks: AsyncIterable<Uint8Array>,
  maximum = MAX_BUFFER,
): AsyncGenerator<string> {
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let parts: Buffer[] = [];
  let size = 0;
  for await (const bytes of chunks) {
    const chunk = Buffer.from(bytes);
    for (let start = 0; start < chunk.length;) {
      const newline = chunk.indexOf(10, start);
      const end = newline === -1 ? chunk.length : newline;
      const part = chunk.subarray(start, end);
      size += part.length;
      if (size >= maximum) throw new Unavailable("stream line exceeds the transport limit");
      if (part.length) parts.push(part);
      if (newline === -1) break;
      const line = decoder.decode(Buffer.concat(parts, size));
      parts = [];
      size = 0;
      yield line.endsWith("\r") ? line.slice(0, -1) : line;
      start = end + 1;
    }
  }
  if (size !== 0) throw new Unavailable("stream ended inside a line");
}

export async function* yuChatStream(input: string, signal?: AbortSignal): AsyncGenerator<string, unknown> {
  signal?.throwIfAborted();
  const child = spawn(yuBinary(), [WIRE_ARG, "ai", "chat", "--request-stdin", "--stream"], {
    stdio: ["pipe", "pipe", "ignore"],
    signal,
    killSignal: "SIGKILL",
  });
  let failure: Error | undefined;
  child.on("error", (error) => { failure = error; });
  child.stdin.on("error", (error: NodeJS.ErrnoException) => {
    if (error.code !== "EPIPE" && !signal?.aborted) {
      failure = error;
      child.kill("SIGKILL");
    }
  });
  const closed = new Promise<number | null>((resolve) => child.once("close", resolve));
  const timeout = setTimeout(() => {
    failure = new Unavailable("App chat stream timed out");
    child.kill("SIGKILL");
  }, DEFAULT_TIMEOUT_MS);
  child.stdin.end(input);
  try {
    let final: string | undefined;
    let total = 0;
    for await (const line of boundedLines(child.stdout)) {
      signal?.throwIfAborted();
      if (final !== undefined) throw new Unavailable("unexpected data after the App chat result");
      const record = decodeWireJson(line);
      if (typeof record === "object" && record !== null && "event" in record) {
        validateAiChatDelta(record);
        total += Buffer.byteLength(record.text);
        if (total > 512 * 1024) throw new Unavailable("App chat stream exceeded its text limit");
        yield record.text;
      } else {
        validateEnvelope(record);
        final = line;
      }
    }
    const status = await closed;
    signal?.throwIfAborted();
    if (failure) throw new Unavailable(failure.message);
    if (final === undefined || status === null) {
      throw new Unavailable("App chat stream ended without a final response");
    }
    return decodeResponse("yu ai chat", final, status);
  } catch (error) {
    signal?.throwIfAborted();
    if (error instanceof SyntaxError || error instanceof TypeError || error instanceof WireDecodeError) {
      throw new Unavailable(`invalid App chat stream: ${error.message}`);
    }
    if (failure) throw new Unavailable(failure.message);
    throw error;
  } finally {
    clearTimeout(timeout);
    if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
    await closed;
  }
}
