// Desktop GUI bootstrap for Yu OS Node apps.
//
// This is the GUI counterpart to the `ai` (model access) and `tools`
// (cross-app verbs) modules. It does NOT wrap a UI toolkit: a Yu OS
// desktop app draws its own window in whatever toolkit it likes ("World
// A"). This module only hands the app the small amount of kernel
// context it receives once kernel-spawned as a GUI, plus the one
// privileged action a GUI commonly wants — summoning the agent overlay.
//
// When an app declares a `desktop` block in app.json, `yu app install`
// writes a launcher whose Exec is `yu app <id> --gui`. Activating it
// routes the launch back through the kernel, which spawns the app with
// YUOS_APP_GUI=1 and YUOS_APP_ID set, so identity/audit/consent apply to
// the GUI exactly as to the headless path.
//
//   const { app, gui } = require("@yuos/sdk");
//
//   app.desktop(() => {
//     startMyWindow(gui.context()); // your toolkit, your loop
//   });
//   app.run();

import { spawn } from "node:child_process";
import { lstatSync } from "node:fs";
import { createConnection } from "node:net";

import * as ai from "./ai";
import * as tools from "./tools";
import {
  type GuiFileRead, type GuiFileWrite, type GuiFileSave, stringifyWireJson, validateGuiFileRead,
  validateGuiFileReadRequest, validateGuiFileReadStatusRequest,
  validateGuiFileWrite, validateGuiFileWriteRequest, validateGuiFileWriteStatusRequest,
  validateGuiFileSave, validateGuiFileSaveReservationRequest,
  type GuiFileCreate, type GuiFileCreateSlot, validateGuiFileCreate,
  validateGuiFileCreateRequest, validateGuiFileCreateStatusRequest,
  validateGuiFileCreateSlot, validateGuiFileCreateReservationRequest,
} from "./generated";
import { yuCallJson } from "./transport";

const ASK_YU_LAUNCHER = "/usr/local/bin/yuos-ask-launcher";
const ASK_YU_PROTOCOL = 1;
const ASK_YU_REQUEST_LIMIT = 32 * 1024;

/** Propose this filtered view's native inode. Pending is not selected content. */
export function requestFileRead(node: number): GuiFileRead {
  const request = { node };
  validateGuiFileReadRequest(request);
  return selectedReadCall("request", request);
}

/** Read the same window/node's Root-owned selection status; never approves. */
export function fileReadStatus(selectionId: string, node: number): GuiFileRead {
  const request = { selection_id: selectionId, node };
  validateGuiFileReadStatusRequest(request);
  return selectedReadCall("status", request);
}

function selectedReadCall(action: string, request: { node: number; selection_id?: string }): GuiFileRead {
  const response = selectedFileCall("__gui-file-read", action, request);
  validateGuiFileRead(response);
  return response;
}

/** Propose independent protected write access; no read permission is implied. */
export function requestFileWrite(node: number): GuiFileWrite {
  const request = { node };
  validateGuiFileWriteRequest(request);
  return selectedWriteCall("request", request);
}

/** Query this same authenticated window and file's write selection. */
export function fileWriteStatus(selectionId: string, node: number): GuiFileWrite {
  const request = { selection_id: selectionId, node };
  validateGuiFileWriteStatusRequest(request);
  return selectedWriteCall("status", request);
}

/** Reserve private scratch for this selected target, never arbitrary siblings. */
export function reserveFileSave(selectionId: string, node: number): GuiFileSave {
  const request = { selection_id: selectionId, node };
  validateGuiFileSaveReservationRequest(request);
  const response = selectedFileCall("__gui-file-write", "reserve", request);
  validateGuiFileSave(response);
  return response;
}

function selectedWriteCall(action: string, request: { node: number; selection_id?: string }): GuiFileWrite {
  const response = selectedFileCall("__gui-file-write", action, request);
  validateGuiFileWrite(response);
  return response;
}

/** Propose one absent literal child of this window's metadata directory. */
export function requestFileCreate(parentNode: number, name: string): GuiFileCreate {
  const request = { parent_node: parentNode, name };
  validateGuiFileCreateRequest(request);
  return selectedCreateCall("request", request);
}

/** Only created is committed creation; selection/reservation is not saved. */
export function fileCreateStatus(selectionId: string, parentNode: number): GuiFileCreate {
  const request = { selection_id: selectionId, parent_node: parentNode };
  validateGuiFileCreateStatusRequest(request);
  return selectedCreateCall("status", request);
}

/** Reserve private scratch for exact one-shot NOREPLACE finalization. */
export function reserveFileCreate(selectionId: string, parentNode: number): GuiFileCreateSlot {
  const request = { selection_id: selectionId, parent_node: parentNode };
  validateGuiFileCreateReservationRequest(request);
  const value = selectedFileCall("__gui-file-create", "reserve", request);
  validateGuiFileCreateSlot(value);
  return value;
}

function selectedCreateCall(
  action: string,
  request: { parent_node: number; name?: string; selection_id?: string },
): GuiFileCreate {
  const value = selectedFileCall("__gui-file-create", action, request);
  validateGuiFileCreate(value);
  return value;
}

function selectedFileCall(
  bridge: string,
  action: string,
  request: Record<string, unknown>,
): unknown {
  return yuCallJson("GUI selected file", [bridge, action, "--request-stdin"],
    30_000, stringifyWireJson(request as never));
}

/**
 * Return `true` when the current invocation is a desktop GUI launch.
 *
 * The bridge sets `YUOS_APP_GUI=1` for the authenticated desktop launch.
 * Command strings are not accepted as a substitute for host context.
 */
export function isGuiLaunch(): boolean {
  return process.env.YUOS_APP_GUI === "1";
}

/** The kernel context handed to a desktop app at launch. */
export class GuiContext {
  readonly appId: string;
  readonly files: string[];
  /** Gated model-access module. */
  readonly ai = ai;
  /** Cross-app verb-call module. */
  readonly tools = tools;

  constructor(appId: string, files: string[]) {
    this.appId = appId;
    this.files = files;
  }

  /**
   * Summon the system "Ask Yu OS" agent overlay through the fixed packaged
   * launcher's authenticated Unix-socket protocol. Pass
   * `hint` to ground the agent's first response in the app's current
   * state without polluting the visible chat transcript.
   *
   * The overlay is detached: it outlives this call and is not tied to
   * the app's stdio or event loop. Resolves once the helper has
   * authenticated and accepted the request; rejects if the
   * overlay binary is missing (e.g. a headless
   * box with no desktop shell).
   */
  async openAgentOverlay(hint?: string): Promise<void> {
    validateAskYuLauncher();
    const child = spawn(ASK_YU_LAUNCHER, ["--protocol", String(ASK_YU_PROTOCOL)], {
      stdio: ["ignore", "pipe", "ignore"],
    });
    try {
      const announcement = await new Promise<string>((resolve, reject) => {
        let settled = false;
        const timer = setTimeout(() => {
          settled = true;
          reject(new Error("Ask Yu OS launcher announcement timed out"));
        }, 5000);
        let line = "";
        child.once("error", (err) => {
          if (settled) return;
          settled = true;
          clearTimeout(timer);
          reject(err);
        });
        child.stdout.on("data", (chunk: Buffer) => {
          if (settled) return;
          line += chunk.toString("utf8");
          if (line.length > 256) {
            settled = true;
            clearTimeout(timer);
            reject(new Error("Ask Yu OS socket announcement is too long"));
            return;
          }
          const newline = line.indexOf("\n");
          if (newline < 0) return;
          settled = true;
          clearTimeout(timer);
          child.stdout.destroy();
          resolve(line.slice(0, newline + 1));
        });
      });
      const match = announcement.match(/^SOCKET 1 @([^\n]+)\n$/);
      if (!match) throw new Error("invalid Ask Yu OS socket announcement");

      const payload = Buffer.from(JSON.stringify({
        protocol: ASK_YU_PROTOCOL,
        app: this.appId,
        hint: hint ?? null,
      }), "utf8");
      if (payload.length > ASK_YU_REQUEST_LIMIT) {
        throw new Error("Ask Yu OS request exceeds the protocol limit");
      }
      await new Promise<void>((resolve, reject) => {
        const socket = createConnection({ path: `\0${match[1]}` });
        let settled = false;
        const timer = setTimeout(() => {
          settled = true;
          socket.destroy();
          reject(new Error("Ask Yu OS launcher readiness timed out"));
        }, 5000);
        let phase: "ready" | "accepted" = "ready";
        let input = Buffer.alloc(0);
        socket.once("error", (err) => {
          if (settled) return;
          settled = true;
          clearTimeout(timer);
          reject(err);
        });
        socket.on("data", (chunk: Buffer) => {
          if (settled) return;
          input = Buffer.concat([input, chunk]);
          if (phase === "ready" && input.length >= 8) {
            if (!input.subarray(0, 8).equals(Buffer.from("READY 1\n"))) {
              settled = true;
              clearTimeout(timer);
              socket.destroy();
              reject(new Error("unexpected Ask Yu OS launcher handshake"));
              return;
            }
            input = input.subarray(8);
            phase = "accepted";
            const header = Buffer.allocUnsafe(4);
            header.writeUInt32BE(payload.length);
            socket.write(Buffer.concat([header, payload]));
          }
          if (phase === "accepted" && input.length >= 11) {
            if (!input.subarray(0, 11).equals(Buffer.from("ACCEPTED 1\n"))) {
              settled = true;
              clearTimeout(timer);
              socket.destroy();
              reject(new Error("unexpected Ask Yu OS acceptance response"));
              return;
            }
            settled = true;
            clearTimeout(timer);
            socket.end();
            resolve();
          }
        });
        socket.once("close", () => {
          if (settled) return;
          settled = true;
          clearTimeout(timer);
          reject(new Error("Ask Yu OS launcher closed the socket"));
        });
      });
      child.unref();
    } catch (error) {
      child.kill();
      throw error;
    }
  }
}

function validateAskYuLauncher(): void {
  for (const path of ["/usr", "/usr/local", "/usr/local/bin"]) {
    const info = lstatSync(path);
    if (info.isSymbolicLink() || !info.isDirectory() || info.uid !== 0 || (info.mode & 0o022) !== 0) {
      throw new Error(`untrusted Ask Yu OS launcher parent: ${path}`);
    }
  }
  const info = lstatSync(ASK_YU_LAUNCHER);
  if (
    info.isSymbolicLink() ||
    !info.isFile() ||
    info.uid !== 0 ||
    (info.mode & 0o111) === 0 ||
    (info.mode & 0o022) !== 0
  ) {
    throw new Error("untrusted Ask Yu OS launcher");
  }
}

/**
 * Build the {@link GuiContext} for the current GUI launch.
 *
 * `appId` is read from `YUOS_APP_ID` (set by the kernel when it spawns
 * the GUI). `files` defaults to the launcher's file arguments, decoded
 * from `YUOS_ARGS_JSON` when not supplied explicitly.
 */
function runPicker(mode: "file" | "save" | "folder", title: string, devEnv: string, name?: string): string | null {
  if (!process.env.YUOS_SESSION && process.env[devEnv] !== undefined) {
    return process.env[devEnv] || null;
  }
  const requestId = Array.from({ length: 16 }, () => Math.floor(Math.random() * 256).toString(16).padStart(2, "0")).join("");
  const payload: Record<string, unknown> = { request_id: requestId, title, modal: true, mode };
  if (name !== undefined) payload.name = name;
  selectedFileCall("__gui-file-picker", "open", payload);
  for (;;) {
    const res = selectedFileCall("__gui-file-picker", "status", { request_id: requestId }) as Record<string, unknown>;
    const status = res?.status;
    if (status === "pending" || status === "cleanup_pending") continue;
    if (status === "selected") {
      const uri = String(res.uri ?? "");
      return uri.startsWith("file://") ? decodeURIComponent(new URL(uri).pathname) : uri;
    }
    if (status === "cancelled") return null;
    throw new Error(String(res?.error || "file picker failed"));
  }
}

/** Open the system file picker and return the selected file path, or `null` if cancelled. */
export function openFile(title = "Open file"): string | null {
  return runPicker("file", title, "YUOS_DEV_OPEN_FILE");
}

/** Open the system Save dialog and return the chosen path, or `null` if cancelled. */
export function saveFile(defaultName: string, title = "Save file"): string | null {
  if (!defaultName || !defaultName.trim()) throw new Error("saveFile: defaultName must be non-empty");
  return runPicker("save", title, "YUOS_DEV_SAVE_FILE", defaultName);
}

/** Open the system folder picker and return the selected directory path, or `null` if cancelled. */
export function selectFolder(title = "Select folder"): string | null {
  return runPicker("folder", title, "YUOS_DEV_SELECT_FOLDER");
}

export function context(files?: string[]): GuiContext {
  const appId = process.env.YUOS_APP_ID;
  if (!appId) throw new Error("YUOS_APP_ID is required for a GUI launch");
  const resolved = files ?? filesFromEnv();
  if (!Array.isArray(resolved) || resolved.some((file) => typeof file !== "string")) {
    throw new Error("GUI files must be an array of strings");
  }
  return new GuiContext(appId, [...resolved]);
}

function filesFromEnv(): string[] {
  const raw = process.env.YUOS_ARGS_JSON;
  if (!raw) return [];
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error("YUOS_ARGS_JSON must be valid JSON");
  }
  if (!Array.isArray(parsed) || parsed.some((file) => typeof file !== "string")) {
    throw new Error("YUOS_ARGS_JSON must be an array of strings");
  }
  return parsed;
}
