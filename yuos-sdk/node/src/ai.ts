// AI helper for Yu OS Node apps.
//
// `chat` is the stable model API and shells out to
// `yu ai chat --app <id>`. The kernel derives `ai.chat` or
// `ai.chat.untrusted` from origin, then runs capability checks, budget,
// safety, and audit.
//
// Apps never name a verb and never pick a model. They describe what
// they want; the gate picks the verb and the machine owner configures
// the one provider/model in /etc/yuos/agent.toml. Importing a provider
// SDK directly (openai, anthropic, ...) is refused at install time by
// `yu app lint` — this module is the only sanctioned route to a model.
//
//   import { ai } from "@yuos/sdk";
//   const res = await ai.chat("Summarise this", { origin: "trusted" });
//   console.log(res.text, res.usage.units);

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
  validateAiInference,
  validateAiInferenceRequest,
  validateBudgetShow,
  type AiBudget,
  type AiInference,
  type AiInferenceRequest,
} from "./generated";

/** Base class for every error this module raises. */
export class AiError extends BridgeError {}
/** The `yu` binary could not be invoked or returned garbage. */
export class AiUnavailable extends AiError {}
/** A gate (capability / origin / budget / safety) refused the call.
 * `payload` is the structured envelope the kernel returned. */
export class AiDenied extends AiError {
  readonly payload: Record<string, unknown>;
  constructor(payload: Record<string, unknown>) {
    super(String(payload["error"] ?? "AI call denied"));
    this.payload = payload;
  }
}
/** The per-app monthly budget was exhausted. */
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

/** A tool call the model proposed but the gate did NOT execute. Apps
 * decide whether to fulfil any by calling `tools.call` with the same
 * name + input; `id` echoes back to the provider on the next turn. */
export interface ProposedToolCall {
  id: string;
  name: string;
  input: WireJsonValue;
}

export interface AiResponse {
  text: string;
  model: string;
  provider: string;
  verb: string;
  usage: Usage;
  budget: Budget;
  review: Review;
  toolCalls: ProposedToolCall[];
  raw: Record<string, WireJsonValue>;
}

export interface ChatOptions {
  /** Declare where the prompt came from; `trusted` is developer-authored text only. */
  origin: "trusted" | "user-input" | "external-content";
  maxUnits?: number;
  system?: string;
  /** App identity. Defaults to `YUOS_APP_ID`. */
  appId?: string;
  /** Catalog tool names the model may *propose*; never executed by the
   * gate — proposals return in `AiResponse.toolCalls`. */
  tools?: string[];
  /** In an MCP handler, pass the call's signal to inherit its deadline and cancellation. */
  signal?: AbortSignal;
}

/** Single-shot chat completion through the kernel's AI gate. */
export async function chat(prompt: string, opts: ChatOptions): Promise<AiResponse> {
  if (!prompt || !prompt.trim()) throw new AiError("chat: prompt must be non-empty");
  if (typeof opts?.origin !== "string" || !opts.origin.trim()) {
    throw new AiError("chat: prompt origin is required");
  }
  const app = opts.appId || process.env.YUOS_APP_ID;
  if (!app) {
    throw new AiError("chat: app_id is required (pass appId or set YUOS_APP_ID)");
  }
  const { origin, maxUnits, system, tools, signal } = opts;
  signal?.throwIfAborted();
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

/** Invoke the owner's configured embedding, speech, image, or decision model. */
export async function infer(
  request: AiInferenceRequest,
  { signal }: { signal?: AbortSignal } = {},
): Promise<AiInference> {
  const input = stringifyWireJson(materializeWireValue(request));
  if (Buffer.byteLength(input) > 720 * 1024) throw new AiError("AI request exceeds the byte limit");
  validateAiInferenceRequest(request);
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
  throw new AiDenied(env);
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
    raw: materializeWireValue(env) as Record<string, WireJsonValue>,
  };
}
