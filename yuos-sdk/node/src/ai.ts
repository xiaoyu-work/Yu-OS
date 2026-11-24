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
  const envApp = process.env.YUOS_APP_ID;
  const isDev = devAiUrl() !== undefined;
  const app = opts.appId || envApp || (isDev ? "dev" : undefined);
  if (!app) {
    throw new AiDenied({ code: "PERMISSION_DENIED", error: "chat: App identity is required" });
  }
  signal?.throwIfAborted();
  if (origin === undefined || images?.length || functions?.length || toolTurns?.length || isDev) {
    if (tools?.length && (origin === undefined || images?.length || functions?.length || toolTurns?.length)) {
      throw new AiInvalidArgs("chat: supply App functions in functions");
    }
    if (opts.appId && (envApp || !isDev) && opts.appId !== envApp) {
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
  if (devAiUrl() !== undefined) {
    return devChat(request, signal);
  }
  try {
    return parseResponse(await yuCallJsonAsync("yu ai chat", ["ai", "chat", "--request-stdin"], signal, input));
  } catch (error) {
    if (error instanceof Denied) raiseForError(error.payload);
    if (error instanceof Unavailable) throw new AiUnavailable(error.message);
    throw error;
  }
}

/** Stream text chunks from the configured chat model. */
export async function* chatStream(
  prompt: string,
  opts: ChatOptions = {},
): AsyncGenerator<string, void, unknown> {
  const baseUrl = devAiUrl();
  if (baseUrl !== undefined) {
    if (!prompt || !prompt.trim()) throw new AiInvalidArgs("chatStream: prompt must be non-empty");
    const messages: Array<Record<string, unknown>> = [];
    if (opts.system != null) messages.push({ role: "system", content: opts.system });
    if (opts.images?.length) {
      const parts: Array<Record<string, unknown>> = [{ type: "text", text: prompt }];
      for (const img of opts.images) {
        parts.push({ type: "image_url", image_url: { url: `data:image/png;base64,${encodeMedia(img, "chatStream: images")}` } });
      }
      messages.push({ role: "user", content: parts });
    } else {
      messages.push({ role: "user", content: prompt });
    }
    const body: Record<string, unknown> = { model: devAiModel(), messages, stream: true };
    if (opts.maxUnits != null) body.max_tokens = Number(opts.maxUnits);
    const raw = Buffer.from(await devPostBytes("/chat/completions", body, opts.signal)).toString("utf8");
    if (raw.trimStart().startsWith("data:")) {
      for (const line of raw.split(/\r?\n/)) {
        const trimmed = line.trim();
        if (!trimmed.startsWith("data:")) continue;
        const dataStr = trimmed.slice(5).trim();
        if (!dataStr || dataStr === "[DONE]") continue;
        try {
          const parsed = JSON.parse(dataStr) as { choices?: Array<{ delta?: { content?: string }; message?: { content?: string } }> };
          const piece = parsed.choices?.[0]?.delta?.content ?? parsed.choices?.[0]?.message?.content;
          if (typeof piece === "string" && piece) yield piece;
        } catch {
          // ignore malformed SSE line
        }
      }
      return;
    }
    const parsed = JSON.parse(raw) as { choices?: Array<{ message?: { content?: string } }> };
    const text = parsed.choices?.[0]?.message?.content;
    if (typeof text === "string" && text) yield text;
    return;
  }
  const reply = await chat(prompt, opts);
  if (reply.output.text) {
    yield reply.output.text;
  }
}

/** Invoke the owner's configured embedding, speech, image, or decision model. */
export async function infer(
  request: AiInferenceRequest,
  { signal }: { signal?: AbortSignal } = {},
): Promise<AiInference> {
  const input = encodeRequest(request, validateAiInferenceRequest);
  let data: unknown;
  if (devAiUrl() !== undefined) {
    data = await devInfer(request, signal);
  } else {
    try {
      data = await yuCallJsonAsync("yu ai infer", ["ai", "infer", "--request-stdin"], signal, input);
    } catch (error) {
      if (error instanceof Denied) raiseForError(error.payload);
      if (error instanceof Unavailable) throw new AiUnavailable(error.message);
      throw error;
    }
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

function devAiUrl(): string | undefined {
  if (process.env.YUOS_SESSION?.trim()) return undefined;
  const url = process.env.YUOS_DEV_AI_URL?.trim();
  return url ? url.replace(/\/+$/, "") : undefined;
}

function devAiModel(kind = "chat"): string {
  const slots: Record<string, [string, string]> = {
    chat: ["YUOS_DEV_AI_CHAT_MODEL", "gpt-6-sol"],
    embed: ["YUOS_DEV_AI_EMBED_MODEL", "text-embedding-3-small"],
    tts: ["YUOS_DEV_AI_TTS_MODEL", "tts-1"],
    stt: ["YUOS_DEV_AI_STT_MODEL", "whisper-1"],
    imagegen: ["YUOS_DEV_AI_IMAGEGEN_MODEL", "dall-e-3"],
    decision: ["YUOS_DEV_AI_DECIDE_MODEL", "gpt-6-sol"],
    decide: ["YUOS_DEV_AI_DECIDE_MODEL", "gpt-6-sol"],
  };
  const [envVar, defaultModel] = slots[kind] ?? ["YUOS_DEV_AI_CHAT_MODEL", "gpt-6-sol"];
  return process.env[envVar]?.trim() || process.env.YUOS_DEV_AI_MODEL?.trim() || defaultModel;
}

function devAiKey(): string | undefined {
  return process.env.YUOS_DEV_AI_KEY?.trim() || undefined;
}

async function devPostBytes(
  path: string,
  payload: Record<string, unknown>,
  signal?: AbortSignal,
): Promise<Uint8Array> {
  signal?.throwIfAborted();
  const baseUrl = devAiUrl();
  if (!baseUrl) throw new AiUnavailable("local dev AI URL is not configured");
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const key = devAiKey();
  if (key) headers["Authorization"] = `Bearer ${key}`;
  let response: Response;
  try {
    response = await fetch(`${baseUrl}${path}`, {
      method: "POST",
      headers,
      body: JSON.stringify(payload),
      signal,
    });
  } catch (error) {
    if (signal?.aborted) throw signal.reason;
    throw new AiUnavailable(`dev AI request failed: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (!response.ok) {
    throw new AiUnavailable(`dev AI HTTP ${response.status}`);
  }
  try {
    return new Uint8Array(await response.arrayBuffer());
  } catch (error) {
    if (signal?.aborted) throw signal.reason;
    throw new AiUnavailable(`dev AI read failed: ${error instanceof Error ? error.message : String(error)}`);
  }
}

async function devPostJson(
  path: string,
  payload: Record<string, unknown>,
  signal?: AbortSignal,
): Promise<Record<string, unknown>> {
  const bytes = await devPostBytes(path, payload, signal);
  let parsed: unknown;
  try {
    parsed = JSON.parse(Buffer.from(bytes).toString("utf8"));
  } catch (error) {
    throw new AiUnavailable(`dev AI returned invalid JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new AiUnavailable("dev AI response must be a JSON object");
  }
  return parsed as Record<string, unknown>;
}

async function devChat(request: AiChatRequest, signal?: AbortSignal): Promise<AiResponse> {
  const model = devAiModel();
  const messages: Array<Record<string, unknown>> = [];
  if (request.system !== undefined) {
    messages.push({ role: "system", content: request.system });
  }
  if (request.images?.length) {
    messages.push({
      role: "user",
      content: [
        { type: "text", text: request.prompt },
        ...request.images.map((b64) => ({
          type: "image_url",
          image_url: { url: `data:image/png;base64,${b64}` },
        })),
      ],
    });
  } else {
    messages.push({ role: "user", content: request.prompt });
  }
  for (const turn of request.tool_turns ?? []) {
    const toolCallsOut = turn.calls.map((c) => {
      const rawArgs = (c as unknown as Record<string, unknown>).input ?? (c as unknown as Record<string, unknown>).arguments ?? {};
      const argsStr = typeof rawArgs === "string" ? rawArgs : stringifyWireJson(materializeWireValue(rawArgs));
      return {
        id: c.id,
        type: "function",
        function: { name: c.name, arguments: argsStr },
      };
    });
    const assistantMsg: Record<string, unknown> = { role: "assistant", tool_calls: toolCallsOut };
    if (turn.text !== undefined) assistantMsg.content = turn.text;
    messages.push(assistantMsg);
    for (const r of turn.results) {
      const raw = r as unknown as Record<string, unknown>;
      const callId = String(raw.id ?? raw.call_id ?? "");
      const content = typeof raw.content === "string"
        ? raw.content
        : stringifyWireJson(materializeWireValue(raw.output ?? raw.content));
      messages.push({ role: "tool", tool_call_id: callId, content });
    }
  }
  const body: Record<string, unknown> = { model, messages };
  if (request.tools?.length) {
    body.tools = request.tools.map((t) => ({
      type: "function",
      function: {
        name: t.name,
        description: t.description,
        parameters: t.parameters,
      },
    }));
  }
  if (request.max_units !== undefined) {
    body.max_tokens = Number(request.max_units);
  }
  const resp = await devPostJson("/chat/completions", body, signal);
  const choices = Array.isArray(resp.choices) ? resp.choices : [];
  const firstChoice = choices[0] && typeof choices[0] === "object" ? (choices[0] as Record<string, unknown>) : {};
  const msg = firstChoice.message && typeof firstChoice.message === "object"
    ? (firstChoice.message as Record<string, unknown>)
    : {};
  const text = typeof msg.content === "string" ? msg.content : "";
  const parsedToolCalls: Array<Record<string, unknown>> = [];
  if (Array.isArray(msg.tool_calls)) {
    for (let i = 0; i < msg.tool_calls.length; i++) {
      const tc = msg.tool_calls[i];
      if (!tc || typeof tc !== "object") throw new AiUnavailable("dev AI tool_call must be an object");
      const tcObj = tc as Record<string, unknown>;
      const fn = tcObj.function;
      if (!fn || typeof fn !== "object" || typeof (fn as Record<string, unknown>).name !== "string") {
        throw new AiUnavailable("dev AI tool_call missing function name");
      }
      const fnObj = fn as Record<string, unknown>;
      const rawArguments = fnObj.arguments ?? "{}";
      let decodedArgs: unknown;
      if (typeof rawArguments === "string") {
        try {
          decodedArgs = rawArguments.trim() ? JSON.parse(rawArguments) : {};
        } catch (error) {
          throw new AiUnavailable(`dev AI tool_call arguments invalid JSON: ${error instanceof Error ? error.message : String(error)}`);
        }
      } else {
        decodedArgs = rawArguments;
      }
      parsedToolCalls.push({
        id: typeof tcObj.id === "string" && tcObj.id ? tcObj.id : `call_${i}`,
        name: fnObj.name,
        input: decodedArgs,
      });
    }
  }
  const usageObj = resp.usage && typeof resp.usage === "object" ? (resp.usage as Record<string, unknown>) : {};
  const totalTokens = typeof usageObj.total_tokens === "number" && Number.isInteger(usageObj.total_tokens) && usageObj.total_tokens >= 0
    ? usageObj.total_tokens
    : 1;
  const promptTokens = typeof usageObj.prompt_tokens === "number" && Number.isInteger(usageObj.prompt_tokens) && usageObj.prompt_tokens >= 0
    ? usageObj.prompt_tokens
    : 0;
  const completionTokens = typeof usageObj.completion_tokens === "number" && Number.isInteger(usageObj.completion_tokens) && usageObj.completion_tokens >= 0
    ? usageObj.completion_tokens
    : 0;
  const actualModel = typeof resp.model === "string" && resp.model.trim() ? resp.model : model;
  const env: Record<string, unknown> = {
    text,
    model: actualModel,
    provider: "dev",
    verb: "ai.chat",
    usage: {
      input_tokens: promptTokens,
      output_tokens: completionTokens,
      units: totalTokens,
    },
    budget: { period: "dev", units_used: 0, units_cap: 0 },
    review: { safety: "strict", prompt_redacted: false },
  };
  if (parsedToolCalls.length > 0) {
    env.tool_calls = parsedToolCalls;
  }
  return parseResponse(env);
}

async function devInfer(
  request: AiInferenceRequest,
  signal?: AbortSignal,
): Promise<Record<string, unknown>> {
  const inp = request.input;
  const kind = inp.kind as string;
  const model = devAiModel(kind);
  let units = 1;
  let actualModel = model;
  let output: Record<string, unknown>;
  if (kind === "embed") {
    const embedInp = inp as Extract<AiInferenceRequest["input"], { kind: "embed" }>;
    const resp = await devPostJson("/embeddings", { model, input: embedInp.text }, signal);
    const data = Array.isArray(resp.data) ? resp.data : [];
    const first = data[0] && typeof data[0] === "object" ? (data[0] as Record<string, unknown>) : undefined;
    if (!first || !Array.isArray(first.embedding)) {
      throw new AiUnavailable("dev AI embeddings response missing data[0].embedding");
    }
    output = { kind: "embed", vector: first.embedding };
    if (typeof resp.model === "string" && resp.model.trim()) actualModel = resp.model;
    const usage = resp.usage && typeof resp.usage === "object" ? (resp.usage as Record<string, unknown>) : undefined;
    if (usage && typeof usage.total_tokens === "number" && Number.isInteger(usage.total_tokens) && usage.total_tokens >= 1) {
      units = usage.total_tokens;
    }
  } else if (kind === "tts") {
    const ttsInp = inp as Extract<AiInferenceRequest["input"], { kind: "tts" }>;
    const raw = await devPostBytes(
      "/audio/speech",
      { model, input: ttsInp.text, voice: "alloy", response_format: "wav" },
      signal,
    );
    let audioB64: string | undefined;
    let format = "wav";
    try {
      const parsed = JSON.parse(Buffer.from(raw).toString("utf8")) as unknown;
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        const obj = parsed as Record<string, unknown>;
        if (typeof obj.audio_base64 === "string") {
          audioB64 = obj.audio_base64;
          if (typeof obj.format === "string") format = obj.format;
        }
      }
    } catch {
      // Raw audio bytes
    }
    if (audioB64 === undefined) {
      if (raw.byteLength === 0) throw new AiUnavailable("dev AI speech response is empty");
      audioB64 = Buffer.from(raw).toString("base64");
    }
    output = { kind: "tts", format, audio_base64: audioB64 };
  } else if (kind === "stt") {
    const sttInp = inp as Extract<AiInferenceRequest["input"], { kind: "stt" }>;
    const resp = await devPostJson(
      "/audio/transcriptions",
      { model, audio_base64: sttInp.audio_base64 },
      signal,
    );
    if (typeof resp.text !== "string") {
      throw new AiUnavailable("dev AI transcription response missing text");
    }
    output = { kind: "stt", text: resp.text };
  } else if (kind === "imagegen") {
    const imgInp = inp as Extract<AiInferenceRequest["input"], { kind: "imagegen" }>;
    const resp = await devPostJson(
      "/images/generations",
      { model, prompt: imgInp.prompt, response_format: "b64_json" },
      signal,
    );
    const data = Array.isArray(resp.data) ? resp.data : [];
    const first = data[0] && typeof data[0] === "object" ? (data[0] as Record<string, unknown>) : undefined;
    if (!first || typeof first.b64_json !== "string") {
      throw new AiUnavailable("dev AI imagegen response missing data[0].b64_json");
    }
    output = { kind: "imagegen", format: "png", image_base64: first.b64_json };
  } else if (kind === "decision" || kind === "decide") {
    const decInp = inp as Extract<AiInferenceRequest["input"], { kind: "decision" }>;
    const userPayload = stringifyWireJson(
      materializeWireValue({ state: decInp.state, questions: decInp.questions }),
    );
    const resp = await devPostJson(
      "/chat/completions",
      {
        model,
        response_format: { type: "json_object" },
        messages: [
          { role: "system", content: 'Return JSON {"answers": {...}} answering each question.' },
          { role: "user", content: userPayload },
        ],
      },
      signal,
    );
    const choices = Array.isArray(resp.choices) ? resp.choices : [];
    const first = choices[0] && typeof choices[0] === "object" ? (choices[0] as Record<string, unknown>) : undefined;
    const msg = first?.message && typeof first.message === "object" ? (first.message as Record<string, unknown>) : undefined;
    if (!msg || typeof msg.content !== "string") {
      throw new AiUnavailable("dev AI decision response missing message content");
    }
    let parsedContent: unknown;
    try {
      parsedContent = JSON.parse(msg.content);
    } catch (error) {
      throw new AiUnavailable(`dev AI decision content is not valid JSON: ${error instanceof Error ? error.message : String(error)}`);
    }
    if (
      !parsedContent ||
      typeof parsedContent !== "object" ||
      Array.isArray(parsedContent) ||
      !(parsedContent as Record<string, unknown>).answers ||
      typeof (parsedContent as Record<string, unknown>).answers !== "object" ||
      Array.isArray((parsedContent as Record<string, unknown>).answers)
    ) {
      throw new AiUnavailable("dev AI decision content missing answers object");
    }
    output = { kind, answers: (parsedContent as Record<string, unknown>).answers };
  } else {
    throw new AiInvalidArgs(`unsupported inference kind: ${kind}`);
  }
  return {
    provider: "dev",
    model: actualModel,
    units,
    output,
  };
}
