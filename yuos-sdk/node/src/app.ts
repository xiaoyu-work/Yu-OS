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

/** Call the function Yu OS started this process for. */
export async function run(): Promise<void> {
  const args: unknown = JSON.parse(process.env.YUOS_ARGS_JSON ?? "[]");
  if (!Array.isArray(args) || args.some((arg) => typeof arg !== "string")) {
    throw new Error("YUOS_ARGS_JSON must be an array of strings");
  }
  if (process.env.YUOS_APP_GUI === "1") {
    if (!desktopHandler) throw new Error("the App has no desktop function");
    await desktopHandler(args);
    return;
  }
  const name = process.env.YUOS_COMMAND ?? "";
  const handler = commands.get(name);
  if (!handler) throw new Error(`the App has no function for command "${name}"`);
  const result = await handler(args);
  if (result !== undefined) process.stdout.write(`${JSON.stringify(result)}\n`);
}
