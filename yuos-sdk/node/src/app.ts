// Mark the functions Yu OS starts for an App's command line and desktop.
//
//   const { app } = require("@yuos/sdk");
//
//   app.command("summarize", (args) => ({ summary: args[0] })); // yu app <id> summarize "text"
//   app.desktop(() => { /* open your window */ });              // App launcher or yu app <id> --gui
//
//   app.run().catch((error) => {
//     console.error(error);
//     process.exitCode = 1;
//   });
//
// Declare the command under `operations` in app.json.

import { mkdirSync, unlinkSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { yuCallJson } from "./transport";

/** Receives a command's validated arguments. */
export type Handler = (args: string[]) => unknown;
/** Opens the App's desktop window. */
export type DesktopHandler = (files?: string[]) => unknown;

const commands = new Map<string, Handler>();
let desktopHandler: DesktopHandler | undefined;
let devClipboard = "";

/** Return the App's private data directory, creating it if needed. */
export function dataDir(): string {
  const envDir = process.env.YUOS_DATA_DIR;
  const dir = envDir
    ? envDir
    : join(
        process.env.XDG_DATA_HOME || join(homedir(), ".local", "share"),
        "yuos-dev",
        "apps",
        process.env.YUOS_APP_ID || "dev",
      );
  mkdirSync(dir, { recursive: true });
  return dir;
}

/** Post a system notification from this App (`ui.notify`). */
export function notify(summary: string, body = ""): Record<string, unknown> {
  if (!summary || !summary.trim()) throw new Error("notify: summary must be non-empty");
  if (!process.env.YUOS_SESSION) {
    return { status: "dev", summary, body };
  }
  const deadline = String(Date.now() + 10_000);
  const payload = JSON.stringify({
    action: "post",
    summary,
    body,
    app_name: process.env.YUOS_APP_ID || "",
    icon: "",
    expire_ms: 0,
    transient: false,
  });
  return yuCallJson(
    "yu __notifications",
    ["__notifications", "request", "--request-stdin", "--deadline", deadline],
    10_000,
    payload,
  ) as Record<string, unknown>;
}

/** Read text from the system clipboard (`clipboard.read`). */
export function clipboardRead(): string {
  if (!process.env.YUOS_SESSION) {
    return process.env.YUOS_DEV_CLIPBOARD ?? devClipboard;
  }
  const res = yuCallJson("yu __clipboard", ["__clipboard", "read"], 10_000) as Record<string, unknown>;
  if (typeof res?.text === "string") return res.text;
  if (typeof res?.content_base64 === "string") {
    return Buffer.from(res.content_base64, "base64").toString("utf8");
  }
  return "";
}

/** Write text to the system clipboard (`clipboard.write`). */
export function clipboardWrite(text: string): void {
  if (typeof text !== "string") throw new Error("clipboardWrite: text must be a string");
  if (!process.env.YUOS_SESSION) {
    devClipboard = text;
    return;
  }
  const scratch = join(dataDir(), ".clipboard-tmp");
  writeFileSync(scratch, text, "utf8");
  try {
    yuCallJson("yu __clipboard", ["__clipboard", "write", "--source", scratch, "--confirm"], 10_000);
  } finally {
    try {
      unlinkSync(scratch);
    } catch {
      // ignore cleanup error
    }
  }
}

/** Load a secret from the Yu OS credential store (`secret.use`). */
export function secret(name: string, namespace = "default"): string {
  if (typeof name !== "string" || !name.trim()) throw new Error("secret: name must be non-empty");
  if (typeof namespace !== "string" || !namespace.trim()) {
    throw new Error("secret: namespace must be non-empty");
  }
  if (!process.env.YUOS_SESSION) {
    const envKey = `YUOS_DEV_SECRET_${name.trim().toUpperCase().replace(/[^A-Z0-9]/g, "_")}`;
    return process.env[envKey] ?? process.env.YUOS_DEV_SECRET ?? "";
  }
  const res = yuCallJson(
    "yu credential load",
    ["credential", "load", name.trim(), "--namespace", namespace.trim()],
    10_000,
  ) as Record<string, unknown>;
  if (typeof res?.value === "string") return res.value;
  throw new Error(`credential ${namespace}/${name} has no value`);
}

export interface PermissionScope {
  path?: string;
  host?: string;
  name?: string;
  selfRef?: string;
  wild?: boolean;
}

/** Check whether the current session may exercise `verb` on a dynamic scope. */
export function checkPermission(verb: string, scope: PermissionScope = {}): Record<string, unknown> {
  if (typeof verb !== "string" || !verb.trim()) {
    throw new Error("checkPermission: verb must be non-empty");
  }
  const supplied: [string, string | undefined][] = [];
  if (scope.path !== undefined) supplied.push(["--path", scope.path]);
  if (scope.host !== undefined) supplied.push(["--host", scope.host]);
  if (scope.name !== undefined) supplied.push(["--name", scope.name]);
  if (scope.selfRef !== undefined) supplied.push(["--self", scope.selfRef]);
  if (scope.wild) supplied.push(["--wild", undefined]);
  if (supplied.length > 1) {
    throw new Error("checkPermission accepts at most one of path / host / name / selfRef / wild");
  }
  if (!process.env.YUOS_SESSION) {
    return { decision: "allow", verb, granted: true };
  }
  const argv = ["__policy", "check", verb];
  if (supplied.length === 1) {
    argv.push(supplied[0][0]);
    if (supplied[0][1] !== undefined) argv.push(supplied[0][1]);
  }
  const res = yuCallJson("yu __policy check", argv, 10_000) as Record<string, unknown>;
  if (!res || typeof res.decision !== "string") {
    throw new Error("policy check response omitted `decision`");
  }
  return res;
}

/** Require that the current session may exercise `verb` on a dynamic scope. */
export function requirePermission(verb: string, scope: PermissionScope = {}): void {
  const decision = checkPermission(verb, scope);
  if (decision.decision !== "allow") {
    throw new Error(typeof decision.summary === "string" ? decision.summary : `permission denied: ${verb}`);
  }
}

/** Run `handler` for `yu app <id> <name>`. Its return value is printed as JSON. */
export function command(nameOrHandler: string | Handler, maybeHandler?: Handler): void {
  const name = typeof nameOrHandler === "function" ? nameOrHandler.name : nameOrHandler;
  const handler = typeof nameOrHandler === "function" ? nameOrHandler : maybeHandler;
  if (!name || !handler) throw new Error("command name and function must be nonempty");
  if (commands.has(name)) throw new Error(`command "${name}" already has a function`);
  commands.set(name, handler);
}

/** Run `handler` to open the App's desktop window. */
export function desktop(handler: DesktopHandler): void {
  if (desktopHandler) throw new Error("the desktop window already has a function");
  desktopHandler = handler;
}

function defaultUnavailable(surface: "cli" | "desktop", commandName?: string): Record<string, unknown> {
  const payload: Record<string, unknown> = {
    available: false,
    surface,
    message:
      surface === "desktop"
        ? "This feature is only available on the command line."
        : "This feature is only available in the desktop app.",
  };
  if (commandName) payload.command = commandName;
  return payload;
}

async function runDesktop(args: string[]): Promise<void> {
  if (desktopHandler) {
    const result = await desktopHandler(args);
    if (result !== undefined) process.stdout.write(`${JSON.stringify(result)}\n`);
    return;
  }
  if (commands.size > 0) {
    process.stdout.write(`${JSON.stringify(defaultUnavailable("desktop"))}\n`);
    return;
  }
  throw new Error("the App has no desktop function");
}

/** Call the function Yu OS started this process for. */
export async function run(): Promise<void> {
  const hasEnvArgs = process.env.YUOS_ARGS_JSON !== undefined && process.env.YUOS_ARGS_JSON !== "";
  const rawArgs: unknown = JSON.parse(hasEnvArgs ? process.env.YUOS_ARGS_JSON! : "[]");
  if (!Array.isArray(rawArgs) || rawArgs.some((arg) => typeof arg !== "string")) {
    throw new Error("YUOS_ARGS_JSON must be an array of strings");
  }
  let args: string[] = rawArgs;
  if (process.env.YUOS_APP_GUI === "1") {
    await runDesktop(args);
    return;
  }
  let name = process.env.YUOS_COMMAND ?? "";
  if (!name) {
    const cliArgs = process.argv.slice(2);
    if (cliArgs[0] === "--gui") {
      await runDesktop(hasEnvArgs ? args : cliArgs.slice(1));
      return;
    }
    if (cliArgs.length > 0) {
      name = cliArgs[0];
      if (!hasEnvArgs) {
        args = cliArgs.slice(1);
      }
    }
  }
  const handler = commands.get(name);
  if (!handler) {
    if (desktopHandler) {
      process.stdout.write(`${JSON.stringify(defaultUnavailable("cli", name || undefined))}\n`);
      return;
    }
    throw new Error(`the App has no function for command "${name}"`);
  }
  const result = await handler(args);
  if (result !== undefined) process.stdout.write(`${JSON.stringify(result)}\n`);
}
