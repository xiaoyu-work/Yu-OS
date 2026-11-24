#!/usr/bin/env node
// Print typed MCP tool bindings: `yuos-tool-types [app.json] > src/tools.ts`.
import { toolTypes } from "./mcp";

try {
  process.stdout.write(toolTypes(process.argv[2]));
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
