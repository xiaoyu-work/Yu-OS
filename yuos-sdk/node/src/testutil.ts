// Test-only helper: installs a fake `yu` binary so transport tests
// run without a real kernel. Not part of the published package
// (excluded from the build tsconfig).

import { mkdtempSync, writeFileSync, chmodSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

export interface FakeYu {
  /** Absolute path to the fake binary; assign to YUOS_KERNEL_BIN. */
  bin: string;
  /** Path the fake writes its received argv to (one arg per line). */
  argvOut: string;
}

export function wireSuccess(data: unknown): string {
  return JSON.stringify({ ok: true, wire_version: 1, data });
}

export function wireSuccessJson(dataJson: string): string {
  return `{"ok":true,"wire_version":1,"data":${dataJson}}`;
}

export function wireError(error: string, code: string): string {
  return JSON.stringify({ ok: false, wire_version: 1, error, code });
}

/**
 * Write a fake `yu` executable that prints `stdout` and exits with
 * `exitCode`, recording the argv it received to a sidecar file. The
 * script is a Node program with a shebang so spawnSync can exec it
 * directly on Linux.
 */
export function installFakeYu(stdout: string, exitCode = 0): FakeYu {
  const dir = mkdtempSync(join(tmpdir(), "yuos-fakecos-"));
  const bin = join(dir, "yu");
  const argvOut = join(dir, "argv.txt");
  const script = `#!/usr/bin/env node
const fs = require("fs");
fs.writeFileSync(${JSON.stringify(argvOut)}, process.argv.slice(2).join("\\n"));
process.stdout.write(${JSON.stringify(stdout)});
process.exit(${exitCode});
`;
  writeFileSync(bin, script);
  chmodSync(bin, 0o755);
  return { bin, argvOut };
}

/** Run `fn` with YUOS_KERNEL_BIN (and optionally YUOS_APP_ID) set, then
 * restore the previous environment. */
export function withCos<T>(
  fake: FakeYu,
  env: Record<string, string | undefined>,
  fn: () => T,
): T {
  const saved: Record<string, string | undefined> = {
    YUOS_KERNEL_BIN: process.env.YUOS_KERNEL_BIN,
    YUOS_APP_ID: process.env.YUOS_APP_ID,
  };
  process.env.YUOS_KERNEL_BIN = fake.bin;
  for (const [k, v] of Object.entries(env)) {
    if (v === undefined) delete process.env[k];
    else process.env[k] = v;
  }
  try {
    return fn();
  } finally {
    for (const [k, v] of Object.entries(saved)) {
      if (v === undefined) delete process.env[k];
      else process.env[k] = v;
    }
  }
}
