// Mark the functions Yu OS starts for an App's command line and desktop.
//
//   const { app } = require("@yuos/sdk");
//
//   app.command("summarize", (args) => ({ summary: args[0] })); // yu app <id> summarize "text"
//   app.desktop((files) => { /* open your window */ });         // App launcher or yu app <id> --gui
//
//   app.run().catch((error) => {
//     console.error(error);
//     process.exitCode = 1;
//   });
//
// Declare the command under `operations` and the window under `desktop` in
// app.json.

/** Receives a command's validated arguments, or the files opened with the App. */
export type Handler = (args: string[]) => unknown;

const commands = new Map<string, Handler>();
let desktopHandler: Handler | undefined;

/** Run `handler` for `yu app <id> <name>`. Its return value is printed as JSON. */
export function command(name: string, handler: Handler): void {
  if (!name) throw new Error("command name must be nonempty");
  if (commands.has(name)) throw new Error(`command "${name}" already has a function`);
  commands.set(name, handler);
}

/** Run `handler` to open the App's desktop window. */
export function desktop(handler: Handler): void {
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
