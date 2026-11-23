// Call the user's configured Yu OS models from an authorized App.
//
//   import { chat } from "@yuos/sdk";
//   const { output } = await chat("Summarize this.");

import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  BridgeError,
  Denied,
  Unavailable,
  yuCallJsonAsync,
} from "./transport";
import {
  WireDecodeError,
  materializeWireValue,
  stringifyWireJson,
  type WireJsonValue,
  validateAi,
  validateAiChatRequest,
  validateAiInference,
  validateAiInferenceRequest,
  validateBudgetShow,
  type AiAppTool,
  type AiBudget,
  type AiChatRequest,
  type AiInference,
  type AiInferenceRequest,
  type AiToolTurn,
  type DecisionInput,
  type ImageOutput,
  type SpeechOutput,
} from "./generated";

/** Base class for every error this module raises. */
export class AiError extends BridgeError {
  constructor(message: string, readonly payload: Record<string, unknown> = {}) {
    super(message);
  }
  get code(): string | undefined {
    return typeof this.payload.code === "string" ? this.payload.code : undefined;
  }
}
/** The OS AI service or the local SDK transport is unavailable. */
export class AiUnavailable extends AiError {}
export class AiInvalidArgs extends AiError {}
/** The App does not have permission to make the call. */
export class AiDenied extends AiError {
  constructor(payload: Record<string, unknown>) {
    super(String(payload["error"] ?? "AI call denied"), payload);
  }
}
/** The available budget could not cover the call. */
export class AiBudgetExceeded extends AiDenied {}
/** The safety pipeline refused the request. */
export class AiSafetyViolation extends AiDenied {}

export interface Usage {
  inputTokens: number;
  outputTokens: number;
  units: number | bigint;
}

export interface Budget {
  period: string;
  unitsUsed: number | bigint;
  unitsCap: number | bigint;
}

export interface BudgetUsage {
  app: string;
  period: string;
  unitsUsed: number | bigint;
}

export interface Review {
  safety: string;
  promptRedacted: boolean;
}

/** A proposal for an App function, not authorization to execute it. */
export interface ProposedToolCall {
  id: string;
  name: string;
  input: WireJsonValue;
}

export interface ModelOptions {
  origin?: "trusted" | "user-input" | "external-content";
  maxUnits?: number;
  signal?: AbortSignal;
}

export interface ModelResponse<Output> {
  provider: string;
  model: string;
  units: number | bigint;
  output: Output;
}

export interface ChatOutput {
  text: string;
  tool_calls: ProposedToolCall[];
  provider_state?: string;
}

export interface Speech {
  format: SpeechOutput["format"];
  audio: Uint8Array;
  sample_rate_hz?: number;
}

export interface Image {
  format: ImageOutput["format"];
  image: Uint8Array;
}

export interface AiResponse extends ModelResponse<ChatOutput> {
  text: string;
  verb: string;
  usage: Usage;
  budget: Budget;
  review: Review;
  toolCalls: ProposedToolCall[];
  providerState?: string;
  raw: Record<string, WireJsonValue>;
}

export interface ChatOptions extends ModelOptions {
  /** Optional App instruction, not the system Agent's prompt. */
  system?: string;
  /** App identity. Defaults to `YUOS_APP_ID`. */
  appId?: string;
  /** @deprecated Catalog names are not accepted. Use {@link chatRequest} for App functions. */
  tools?: string[];
  /** PNG, JPEG or WebP image bytes; Yu OS detects each format. */
  images?: Uint8Array[];
  functions?: AiAppTool[];
  toolTurns?: AiToolTurn[];
}

/** Single-shot chat completion through the kernel's AI gate. */
export async function chat(prompt: string, opts: ChatOptions = {}): Promise<AiResponse> {
  if (typeof prompt !== "string" || !prompt.trim()) throw new AiInvalidArgs("chat: prompt must be non-empty");
  const { origin, maxUnits, system, tools, signal, images, functions, toolTurns } = opts;
  if (origin !== undefined && (typeof origin !== "string" || !origin.trim())) {
    throw new AiInvalidArgs("chat: invalid origin");
  }
  const app = opts.appId || process.env.YUOS_APP_ID;
  if (!app) {
    throw new AiDenied({ code: "PERMISSION_DENIED", error: "chat: App identity is required" });
  }
  signal?.throwIfAborted();
  if (origin === undefined || images?.length || functions?.length || toolTurns?.length) {
    if (tools?.length) throw new AiInvalidArgs("chat: supply App functions in functions");
    if (app !== process.env.YUOS_APP_ID) {
      throw new AiDenied({ code: "PERMISSION_DENIED", error: "chat: App identity does not match the session" });
    }
    const request: AiChatRequest = { origin: origin ?? "app", prompt };
    if (system != null) request.system = system;
    if (maxUnits != null) request.max_units = maxUnits;
    if (images?.length) request.images = images.map((image) => encodeMedia(image, "chat: images"));
    if (functions?.length) request.tools = functions;
    if (toolTurns?.length) request.tool_turns = toolTurns;
    return chatRequest(request, { signal });
  }
  const argv = ["ai", "chat", "--app", app, "--origin", origin];
  const privateDir = await mkdtemp(join(tmpdir(), "yuos-ai-"));
  try {
    const promptPath = join(privateDir, "prompt");
    await writeFile(promptPath, prompt, { encoding: "utf8", mode: 0o600, flag: "wx" });
    argv.push("--prompt-file", promptPath);
    if (maxUnits != null) argv.push("--max-units", String(maxUnits));
    if (system != null) {
      const systemPath = join(privateDir, "system");
      await writeFile(systemPath, system, { encoding: "utf8", mode: 0o600, flag: "wx" });
      argv.push("--system-file", systemPath);
    }
    if (tools?.length) argv.push("--tools", tools.join(","));

    let data: unknown;
    try {
      data = await yuCallJsonAsync("yu ai chat", argv, signal);
    } catch (e) {
      if (e instanceof Denied) raiseForError(e.payload);
      if (e instanceof Unavailable) throw new AiUnavailable(e.message);
      throw e;
    }
    return parseResponse(data);
  } finally {
    await rm(privateDir, { recursive: true, force: true });
  }
}

/** Call the configured chat model with optional App functions and prior results. */
export async function chatRequest(
  request: AiChatRequest,
  { signal }: { signal?: AbortSignal } = {},
): Promise<AiResponse> {
  const input = encodeRequest(request, validateAiChatRequest);
  try {
    return parseResponse(await yuCallJsonAsync("yu ai chat", ["ai", "chat", "--request-stdin"], signal, input));
  } catch (error) {
    if (error instanceof Denied) raiseForError(error.payload);
    if (error instanceof Unavailable) throw new AiUnavailable(error.message);
    throw error;
  }
}

/** Invoke the owner's configured embedding, speech, image, or decision model. */
export async function infer(
  request: AiInferenceRequest,
  { signal }: { signal?: AbortSignal } = {},
): Promise<AiInference> {
  const input = encodeRequest(request, validateAiInferenceRequest);
  let data: unknown;
  try {
    data = await yuCallJsonAsync("yu ai infer", ["ai", "infer", "--request-stdin"], signal, input);
  } catch (error) {
    if (error instanceof Denied) raiseForError(error.payload);
    if (error instanceof Unavailable) throw new AiUnavailable(error.message);
    throw error;
  }
  try {
    validateAiInference(data);
  } catch (error) {
    if (error instanceof WireDecodeError) throw new AiUnavailable(error.message);
    throw error;
  }
  if (data.output.kind !== request.input.kind) {
    throw new AiUnavailable("AI inference reply does not match the request");
  }
  return data;
}

function encodeRequest(request: unknown, validate: (value: unknown) => void): string {
  try {
    validate(request);
    const input = stringifyWireJson(materializeWireValue(request));
    if (Buffer.byteLength(input) > 720 * 1024) throw new AiInvalidArgs("AI request exceeds the byte limit");
    return input;
  } catch (error) {
    if (error instanceof WireDecodeError || error instanceof TypeError || error instanceof RangeError) {
      throw new AiInvalidArgs(error.message);
    }
    throw error;
  }
}

function encodeMedia(data: Uint8Array, label: string): string {
  if (!(data instanceof Uint8Array)) throw new AiInvalidArgs(`${label} must be a Uint8Array`);
  return Buffer.from(data.buffer, data.byteOffset, data.byteLength).toString("base64");
}

function decodeMedia(value: string): Uint8Array {
  const bytes = Buffer.from(value, "base64");
  if (bytes.toString("base64") !== value) throw new AiUnavailable("AI inference reply has invalid media");
  return new Uint8Array(bytes);
}

type ModelInput = AiInferenceRequest["input"];
type ModelKind = ModelInput["kind"];
type TaggedOutput<K extends ModelKind> = Extract<AiInference["output"], { kind: K }>;

function matchesKind<K extends ModelKind>(
  output: AiInference["output"], kind: K,
): output is TaggedOutput<K> {
  return output.kind === kind;
}

async function runModel<K extends ModelKind>(
  input: ModelInput & { kind: K }, opts: ModelOptions,
): Promise<ModelResponse<Omit<TaggedOutput<K>, "kind">>> {
  const request: AiInferenceRequest = { origin: opts.origin ?? "app", input };
  if (opts.maxUnits != null) request.max_units = opts.maxUnits;
  const { output, ...reply } = await infer(request, { signal: opts.signal });
  if (!matchesKind(output, input.kind)) {
    throw new AiUnavailable("AI inference reply does not match the request");
  }
  const { kind: _kind, ...payload } = output;
  return { ...reply, output: payload };
}

export function embed(text: string, opts: ModelOptions = {}) {
  return runModel({ kind: "embed", text }, opts);
}

export async function tts(text: string, opts: ModelOptions = {}): Promise<ModelResponse<Speech>> {
  const { output: { audio_base64, ...output }, ...reply } = await runModel({ kind: "tts", text }, opts);
  return { ...reply, output: { ...output, audio: decodeMedia(audio_base64) } };
}

/** Transcribe WAV, MP3 or Ogg audio bytes; Yu OS detects the format. */
export async function stt(audio: Uint8Array, opts: ModelOptions = {}) {
  return runModel({ kind: "stt", audio_base64: encodeMedia(audio, "stt: audio") }, opts);
}

export async function imagegen(prompt: string, opts: ModelOptions = {}): Promise<ModelResponse<Image>> {
  const { output: { image_base64, ...output }, ...reply } = await runModel({ kind: "imagegen", prompt }, opts);
  return { ...reply, output: { ...output, image: decodeMedia(image_base64) } };
}

export function decide(state: DecisionInput["state"], questions: DecisionInput["questions"], opts: ModelOptions = {}) {
  return runModel({ kind: "decision", state, questions }, opts);
}

/** Open Applications settings after a user action. This does not authorize AI. */
export async function openPermissionSettings(): Promise<void> {
  try {
    await yuCallJsonAsync("yu ai settings", ["ai", "settings"]);
  } catch (error) {
    if (error instanceof Denied) raiseForError(error.payload);
    if (error instanceof Unavailable) throw new AiUnavailable(error.message);
    throw error;
  }
}

/** Current-period usage for an app; the CLI does not report a cap. */
export async function budget(
  appId?: string,
  { signal }: { signal?: AbortSignal } = {},
): Promise<BudgetUsage> {
  const app = appId || process.env.YUOS_APP_ID;
  if (!app) throw new AiError("budget: app_id is required");
  let data: unknown;
  try {
    data = await yuCallJsonAsync("yu agent budget show", ["agent", "budget", "show", app], signal);
  } catch (error) {
    if (error instanceof Denied || error instanceof Unavailable) {
      throw new AiUnavailable(error.message);
    }
    throw error;
  }
  try {
    validateBudgetShow(data);
  } catch (error) {
    if (error instanceof WireDecodeError) {
      throw new AiUnavailable(`budget response decode failed: ${error.message}`);
    }
    throw error;
  }
  return { app: data.app, period: data.period, unitsUsed: data.units_used };
}

function raiseForError(env: Record<string, unknown>): never {
  const code = String(env["code"]);
  if (code === "BUDGET_EXCEEDED") throw new AiBudgetExceeded(env);
  if (code === "SAFETY_VIOLATION") throw new AiSafetyViolation(env);
  const message = String(env["error"]);
  if (code === "KERNEL_UNAVAILABLE") throw new AiUnavailable(message, env);
  if (code === "INVALID_ARGS") throw new AiInvalidArgs(message, env);
  if (["PERMISSION_DENIED", "UNKNOWN_APP", "UNKNOWN_VERB", "DENIED"].includes(code)) throw new AiDenied(env);
  throw new AiError(message, env);
}

function parseBudget(blk: AiBudget): Budget {
  return {
    period: blk.period,
    unitsUsed: blk.units_used,
    unitsCap: blk.units_cap,
  };
}

function parseResponse(env: unknown): AiResponse {
  try {
    validateAi(env);
  } catch (error) {
    if (error instanceof WireDecodeError) {
      throw new AiUnavailable(`ai response decode failed: ${error.message}`);
    }
    throw error;
  }
  const usage = env.usage;
  const review = env.review;
  const toolCalls: ProposedToolCall[] = (env.tool_calls ?? []).map((toolCall) => ({
    id: toolCall.id,
    name: toolCall.name,
    input: materializeWireValue(toolCall.input),
  }));
  return {
    text: env.text,
    model: env.model,
    provider: env.provider,
    units: usage.units,
    output: { text: env.text, tool_calls: toolCalls, provider_state: env.provider_state },
    verb: env.verb,
    usage: {
      inputTokens: usage.input_tokens,
      outputTokens: usage.output_tokens,
      units: usage.units,
    },
    budget: parseBudget(env.budget),
    review: {
      safety: review.safety,
      promptRedacted: review.prompt_redacted,
    },
    toolCalls,
    providerState: env.provider_state,
    raw: materializeWireValue(env) as Record<string, WireJsonValue>,
  };
}
