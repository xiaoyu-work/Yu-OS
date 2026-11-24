(function () {
  var API_DATA = {
    python: {
      title: "yuos_sdk (Python API Reference)",
      install: "pip install yuos-sdk",
      lead: "Official Python SDK for Yu OS Apps (Python 3.10+). Provides @command and @desktop entry decorators, system AI calls, native file/folder dialogs, MCP server bindings, and OS helpers.",
      modules: [
        {
          id: "mod-app",
          name: "yuos_sdk.app",
          summary: "Application entrypoints (@command, @desktop, run) and OS helpers (data_dir, notify, clipboard, secret, permissions).",
          items: [
            {
              id: "app-command",
              qual: "@yuos_sdk.app.",
              name: "command",
              paramsSig: "(name_or_fn)",
              retSig: "Callable[[list[str]], Any]",
              desc: "Decorator that registers a function as a command-line operation invoked via yu app <id> <name> [args...]. Can be used bare as @command (uses the Python function's __name__) or with an explicit command name @command(\"name\").",
              params: [
                ["name_or_fn", "Callable | str", "Either the decorated handler function (when used as @command) or a non-empty operation name string matching operations.<name> in app.json."],
                ["args (handler arg)", "list[str]", "Passed to the decorated function at runtime: the validated CLI argument strings after the command name."]
              ],
              returns: "The decorated function. Its return value (dict, list, str, int, bool, or None) is serialized as JSON to stdout.",
              rtype: "Callable[[list[str]], Any]",
              raises: "ValueError – If the command name is empty or registered twice.",
              example: "from yuos_sdk.app import command\n\n@command\ndef summarize(args: list[str]) -> dict:\n    return {\"summary\": args[0]}"
            },
            {
              id: "app-desktop",
              qual: "@yuos_sdk.app.",
              name: "desktop",
              paramsSig: "(fn)",
              retSig: "Callable[[], Any]",
              desc: "Decorator that registers the zero-argument entry function called when the App is launched from the desktop launcher or via yu app <id> --gui. If an App only defines @command and a user launches --gui (or vice versa), the SDK automatically returns a structured unavailable-surface JSON response.",
              params: [
                ["fn", "Callable[[], Any]", "Zero-argument function that starts your GUI window loop (Qt, GTK, Tkinter, etc.)."]
              ],
              returns: "The decorated desktop entry function.",
              rtype: "Callable[[], Any]",
              raises: "ValueError – If a default @desktop function is registered more than once.",
              example: "from yuos_sdk.app import desktop\n\n@desktop\ndef main() -> None:\n    # Open your Qt / GTK / Tkinter window\n    ..."
            },
            {
              id: "app-run",
              qual: "yuos_sdk.app.",
              name: "run",
              paramsSig: "(argv=None)",
              retSig: "Any",
              desc: "Dispatch the current process invocation to the registered @command or @desktop function. In Python, calling run() explicitly is optional because yuos_sdk.app automatically dispatches when main.py finishes loading.",
              params: [
                ["argv", "list[str] | None, optional", "Optional argument vector override for unit tests. Default: None (reads YUOS_COMMAND / YUOS_ARGS_JSON / sys.argv)."]
              ],
              returns: "The return value of the invoked handler function.",
              rtype: "Any",
              raises: "RuntimeError – If no matching command or desktop handler is registered.",
              example: "from yuos_sdk.app import run\n\nif __name__ == \"__main__\":\n    run()"
            },
            {
              id: "app-data-dir",
              qual: "yuos_sdk.app.",
              name: "data_dir",
              paramsSig: "()",
              retSig: "pathlib.Path",
              desc: "Return this App's isolated private data directory, creating it if it does not exist. Uses $YUOS_DATA_DIR inside a Yu OS session, or ~/.local/share/yuos-dev/apps/<id> during local development. Requires no permissions in app.json.",
              params: [],
              returns: "Path object pointing to the existing writable per-App directory.",
              rtype: "pathlib.Path",
              raises: "OSError – If the directory cannot be created.",
              example: "from yuos_sdk.app import data_dir\n\ndb_file = data_dir() / \"state.json\"\ndb_file.write_text('{\"count\": 1}', encoding=\"utf-8\")"
            },
            {
              id: "app-notify",
              qual: "yuos_sdk.app.",
              name: "notify",
              paramsSig: "(summary, body=\"\")",
              retSig: "dict[str, Any]",
              desc: "Post a desktop/system notification from this App. Requires ui.notify in app.json when running inside a Yu OS session.",
              params: [
                ["summary", "str", "Notification title (must be non-empty)."],
                ["body", "str, optional", "Notification body text. Default: \"\"."]
              ],
              returns: "Notification status dictionary from the OS notification broker.",
              rtype: "dict[str, Any]",
              raises: "ValueError – If summary is empty. KernelDenied – If ui.notify permission is not granted.",
              example: "from yuos_sdk.app import notify\n\nnotify(\"Export complete\", \"Saved report.pdf\")"
            },
            {
              id: "app-clipboard-read",
              qual: "yuos_sdk.app.",
              name: "clipboard_read",
              paramsSig: "()",
              retSig: "str",
              desc: "Read plain text from the system clipboard (requires clipboard.read in app.json). Outside YUOS_SESSION, returns $YUOS_DEV_CLIPBOARD or the last value written via clipboard_write().",
              params: [],
              returns: "Current UTF-8 text on the clipboard, or \"\" if empty.",
              rtype: "str",
              raises: "KernelDenied – If clipboard.read permission is denied.",
              example: "from yuos_sdk.app import clipboard_read\n\ntext = clipboard_read()"
            },
            {
              id: "app-clipboard-write",
              qual: "yuos_sdk.app.",
              name: "clipboard_write",
              paramsSig: "(text)",
              retSig: "None",
              desc: "Write plain text to the system clipboard (requires clipboard.write in app.json).",
              params: [
                ["text", "str", "UTF-8 string to place on the system clipboard."]
              ],
              returns: "None",
              rtype: "None",
              raises: "ValueError – If text is not a str. KernelDenied – If clipboard.write is denied.",
              example: "from yuos_sdk.app import clipboard_write\n\nclipboard_write(\"Copied from Yu OS App\")"
            },
            {
              id: "app-secret",
              qual: "yuos_sdk.app.",
              name: "secret",
              paramsSig: "(name, namespace=\"default\")",
              retSig: "str",
              desc: "Load an encrypted credential or API token from the Yu OS Credential Store (requires secret.use on the named secret). Outside YUOS_SESSION, reads $YUOS_DEV_SECRET_<UPPER_NAME> or $YUOS_DEV_SECRET.",
              params: [
                ["name", "str", "Secret identifier in the credential store (e.g. \"GOOGLE_ACCESS_TOKEN\")."],
                ["namespace", "str, optional", "Credential namespace. Default: \"default\"."]
              ],
              returns: "Decrypted secret string.",
              rtype: "str",
              raises: "ValueError – If name or namespace is empty. RuntimeError / KernelDenied – If the secret is missing or unauthorized.",
              example: "from yuos_sdk.app import secret\n\ntoken = secret(\"GITHUB_TOKEN\")"
            },
            {
              id: "app-require-permission",
              qual: "yuos_sdk.app.",
              name: "require_permission",
              paramsSig: "(verb, *, path=None, host=None, name=None, self_ref=None, wild=False)",
              retSig: "None",
              desc: "Verify at runtime that the current session holds verb for a dynamically computed scope (e.g. checking net.dial on an HTTP redirect target). Use check_permission(...) with the same arguments to return the raw decision dict without raising.",
              params: [
                ["verb", "str", "Capability verb to check (e.g. \"net.dial\", \"fs.read\")."],
                ["path", "str | None, optional", "Filesystem path scope. Supply at most one scope keyword."],
                ["host", "str | None, optional", "Network host or host:port scope."],
                ["name", "str | None, optional", "Named resource scope."],
                ["self_ref", "str | None, optional", "App self-reference scope."],
                ["wild", "bool, optional", "Check wildcard scope. Default: False."]
              ],
              returns: "None (raises PermissionError if denied).",
              rtype: "None",
              raises: "PermissionError – If the kernel denies the requested capability.",
              example: "from yuos_sdk.app import require_permission\n\nrequire_permission(\"net.dial\", host=\"api.example.com:443\")"
            }
          ]
        },
        {
          id: "mod-ai",
          name: "yuos_sdk.ai",
          summary: "System AI calls (chat, chat_stream, embed, tts, stt, imagegen, decide, budget) routed through the owner's configured models.",
          items: [
            {
              id: "ai-chat",
              qual: "yuos_sdk.ai.",
              name: "chat",
              paramsSig: "(prompt, *, system=None, images=None, max_units=None, functions=None, tool_turns=None)",
              retSig: "ModelResponse[ChatOutput]",
              desc: "Call the user's configured chat model. Declare \"ai\": {} and \"needs\": [\"ai.chat\"] in app.json.",
              params: [
                ["prompt", "str", "Non-empty prompt text sent to the model."],
                ["system", "str | None, optional", "Optional system instruction for the model. Default: None."],
                ["images", "Sequence[bytes] | None, optional", "Up to 4 raw image byte buffers (PNG, JPEG, or WebP auto-detected from magic bytes; max 512 KiB combined). Default: None."],
                ["max_units", "int | None, optional", "Optional per-call budget ceiling. Default: None."],
                ["functions", "Sequence[Mapping[str, Any]] | None, optional", "Optional JSON Schema function definitions for tool calling. Default: None."],
                ["tool_turns", "Sequence[Mapping[str, Any]] | None, optional", "Prior model tool calls and App execution results when continuing a tool-call turn. Default: None."]
              ],
              returns: "ModelResponse with .provider (str), .model (str), .units (int), and .output (ChatOutput with .text: str and .tool_calls: list[ProposedToolCall]).",
              rtype: "ModelResponse[ChatOutput]",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiSafetyViolation, AiUnavailable",
              example: "from yuos_sdk.ai import chat\n\nreply = chat(\"Explain quantum tunneling in one sentence.\", system=\"Be concise.\")\nprint(reply.output.text)"
            },
            {
              id: "ai-chat-stream",
              qual: "yuos_sdk.ai.",
              name: "chat_stream",
              paramsSig: "(prompt, *, system=None, images=None, max_units=None)",
              retSig: "Iterator[str]",
              desc: "Stream text chunks from the configured chat model as they are generated (\"needs\": [\"ai.chat\"]).",
              params: [
                ["prompt", "str", "Non-empty prompt text."],
                ["system", "str | None, optional", "Optional system instruction. Default: None."],
                ["images", "Sequence[bytes] | None, optional", "Optional raw PNG/JPEG/WebP image bytes. Default: None."],
                ["max_units", "int | None, optional", "Optional per-call budget ceiling. Default: None."]
              ],
              returns: "Iterator yielding incremental str text chunks.",
              rtype: "Iterator[str]",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiSafetyViolation, AiUnavailable",
              example: "from yuos_sdk.ai import chat_stream\n\nfor chunk in chat_stream(\"Write a haiku about Linux.\"):\n    print(chunk, end=\"\", flush=True)"
            },
            {
              id: "ai-embed",
              qual: "yuos_sdk.ai.",
              name: "embed",
              paramsSig: "(text, *, max_units=None)",
              retSig: "ModelResponse[Embedding]",
              desc: "Compute a semantic embedding vector using the owner's configured embedding model (\"needs\": [\"ai.embed\"]).",
              params: [
                ["text", "str", "Non-empty text to embed."],
                ["max_units", "int | None, optional", "Optional per-call unit ceiling. Default: None."]
              ],
              returns: "ModelResponse whose .output.vector is a list[float] embedding vector.",
              rtype: "ModelResponse[Embedding]",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiUnavailable",
              example: "from yuos_sdk.ai import embed\n\nvec = embed(\"Yu OS native desktop\").output.vector"
            },
            {
              id: "ai-tts",
              qual: "yuos_sdk.ai.",
              name: "tts",
              paramsSig: "(text, *, max_units=None)",
              retSig: "ModelResponse[Speech]",
              desc: "Synthesize speech audio bytes from text (\"needs\": [\"ai.audio.tts\"]).",
              params: [
                ["text", "str", "Text to synthesize into speech."],
                ["max_units", "int | None, optional", "Optional per-call unit ceiling. Default: None."]
              ],
              returns: "ModelResponse whose .output is Speech(format: str, audio: bytes, sample_rate_hz: int | None).",
              rtype: "ModelResponse[Speech]",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiUnavailable",
              example: "from pathlib import Path\nfrom yuos_sdk.ai import tts\n\nspeech = tts(\"Welcome to Yu OS.\").output\nPath(\"welcome.wav\").write_bytes(speech.audio)"
            },
            {
              id: "ai-stt",
              qual: "yuos_sdk.ai.",
              name: "stt",
              paramsSig: "(audio, *, max_units=None)",
              retSig: "ModelResponse[Transcription]",
              desc: "Transcribe WAV, MP3, or Ogg audio bytes into text (\"needs\": [\"ai.audio.stt\"]). Format is detected automatically from magic bytes.",
              params: [
                ["audio", "bytes", "Raw WAV, MP3, or Ogg audio bytes."],
                ["max_units", "int | None, optional", "Optional per-call unit ceiling. Default: None."]
              ],
              returns: "ModelResponse whose .output.text is the transcribed str.",
              rtype: "ModelResponse[Transcription]",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiUnavailable",
              example: "from pathlib import Path\nfrom yuos_sdk.ai import stt\n\ntext = stt(Path(\"memo.wav\").read_bytes()).output.text"
            },
            {
              id: "ai-imagegen",
              qual: "yuos_sdk.ai.",
              name: "imagegen",
              paramsSig: "(prompt, *, max_units=None)",
              retSig: "ModelResponse[Image]",
              desc: "Generate an image from a text prompt (\"needs\": [\"ai.image.generate\"]).",
              params: [
                ["prompt", "str", "Text description of the image to generate."],
                ["max_units", "int | None, optional", "Optional per-call unit ceiling. Default: None."]
              ],
              returns: "ModelResponse whose .output is Image(format: str, image: bytes).",
              rtype: "ModelResponse[Image]",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiSafetyViolation, AiUnavailable",
              example: "from pathlib import Path\nfrom yuos_sdk.ai import imagegen\n\nimg = imagegen(\"Minimalist mountain wallpaper\").output\nPath(f\"out.{img.format}\").write_bytes(img.image)"
            },
            {
              id: "ai-decide",
              qual: "yuos_sdk.ai.",
              name: "decide",
              paramsSig: "(state, questions, *, max_units=None)",
              retSig: "ModelResponse[Decision]",
              desc: "Evaluate structured state and return typed answers to a map of questions (\"needs\": [\"ai.decide\"]).",
              params: [
                ["state", "WireJsonValue", "JSON-serializable state object."],
                ["questions", "Mapping[str, WireJsonValue]", "Map of question IDs to question specs."],
                ["max_units", "int | None, optional", "Optional per-call unit ceiling. Default: None."]
              ],
              returns: "ModelResponse whose .output.answers maps each question ID to its answer.",
              rtype: "ModelResponse[Decision]",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiUnavailable",
              example: "from yuos_sdk.ai import decide\n\nres = decide({\"cpu_temp\": 88}, {\"throttle\": \"Should we throttle CPU?\"})\nprint(res.output.answers)"
            },
            {
              id: "ai-budget",
              qual: "yuos_sdk.ai.",
              name: "budget",
              paramsSig: "(app_id=None)",
              retSig: "BudgetUsage",
              desc: "Query this App's AI budget usage for the current UTC month.",
              params: [
                ["app_id", "str | None, optional", "App ID override. Default: None (uses $YUOS_APP_ID)."]
              ],
              returns: "BudgetUsage(app: str, period: str, units_used: int).",
              rtype: "BudgetUsage",
              raises: "AiUnavailable, AiError",
              example: "from yuos_sdk.ai import budget\n\nprint(budget().units_used)"
            }
          ]
        },
        {
          id: "mod-gui",
          name: "yuos_sdk.gui",
          summary: "System file/folder pickers (open_file, save_file, select_folder), desktop window context, and Ask Yu OS overlay.",
          items: [
            {
              id: "gui-open-file",
              qual: "yuos_sdk.gui.",
              name: "open_file",
              paramsSig: "(title=\"Open file\")",
              retSig: "str | None",
              desc: "Open the native system file picker dialog. Selecting a file automatically grants read access to that file for the running window (no needs declaration required in app.json).",
              params: [
                ["title", "str, optional", "Dialog window title. Default: \"Open file\"."]
              ],
              returns: "Absolute path to the selected file, or None if the user cancelled.",
              rtype: "str | None",
              raises: "RuntimeError – If not running in a GUI session (unless $YUOS_DEV_OPEN_FILE is set).",
              example: "from yuos_sdk import gui\n\npath = gui.open_file(\"Select a PDF\")\nif path:\n    data = open(path, \"rb\").read()"
            },
            {
              id: "gui-save-file",
              qual: "yuos_sdk.gui.",
              name: "save_file",
              paramsSig: "(default_name=\"untitled.txt\", title=\"Save file\")",
              retSig: "str | None",
              desc: "Open the native system Save file dialog (requires ui.prompt in app.json).",
              params: [
                ["default_name", "str, optional", "Suggested file name. Default: \"untitled.txt\"."],
                ["title", "str, optional", "Dialog window title. Default: \"Save file\"."]
              ],
              returns: "Chosen destination file path, or None if cancelled.",
              rtype: "str | None",
              raises: "KernelDenied – If ui.prompt is not granted.",
              example: "from yuos_sdk import gui\n\ndest = gui.save_file(\"report.txt\", \"Export Report\")"
            },
            {
              id: "gui-select-folder",
              qual: "yuos_sdk.gui.",
              name: "select_folder",
              paramsSig: "(title=\"Select folder\")",
              retSig: "str | None",
              desc: "Open the native system folder selection dialog (requires ui.prompt in app.json).",
              params: [
                ["title", "str, optional", "Dialog window title. Default: \"Select folder\"."]
              ],
              returns: "Selected directory path, or None if cancelled.",
              rtype: "str | None",
              raises: "KernelDenied – If ui.prompt is not granted.",
              example: "from yuos_sdk import gui\n\nfolder = gui.select_folder(\"Choose workspace folder\")"
            },
            {
              id: "gui-context",
              qual: "yuos_sdk.gui.",
              name: "context",
              paramsSig: "(files=None)",
              retSig: "GuiContext",
              desc: "Return the desktop GUI launch context containing app_id, any associated files passed by the desktop launcher, and open_agent_overlay(hint=None).",
              params: [
                ["files", "Sequence[str] | None, optional", "Optional explicit file list override. Default: None (reads $YUOS_ARGS_JSON)."]
              ],
              returns: "GuiContext(app_id: str, files: tuple[str, ...]) with method .open_agent_overlay(hint: str | None = None).",
              rtype: "GuiContext",
              raises: "GuiLaunchError – If $YUOS_APP_ID is unset.",
              example: "from yuos_sdk import gui\n\nctx = gui.context()\nctx.open_agent_overlay(hint=\"Summarize current document\")"
            }
          ]
        },
        {
          id: "mod-mcp",
          name: "yuos_sdk.mcp",
          summary: "Manifest-bound MCP server runtime exposing App tools to the built-in Yu OS System Agent.",
          items: [
            {
              id: "mcp-app",
              qual: "class yuos_sdk.mcp.",
              name: "App",
              paramsSig: ".from_manifest(manifest_path=None)",
              retSig: "App",
              desc: "Load app.json, register handlers for every tool declared in mcp.tools via @app.tool(name), and serve JSON-RPC over stdio with app.serve().",
              params: [
                ["manifest_path", "str | Path | None, optional", "Path to app.json. Default: None (reads $YUOS_APP_MANIFEST or ./app.json)."]
              ],
              returns: "Configured MCP App server instance with .tool(name) decorator and .serve() method.",
              rtype: "yuos_sdk.mcp.App",
              raises: "ManifestError – If app.json is missing or invalid.",
              example: "from yuos_sdk.mcp import App\n\napp = App.from_manifest()\n\n@app.tool(\"notes.add\")\ndef add_note(title: str) -> dict:\n    return {\"saved\": title}\n\nif __name__ == \"__main__\":\n    app.serve()"
            },
            {
              id: "mcp-current-context",
              qual: "yuos_sdk.mcp.",
              name: "current_context",
              paramsSig: "()",
              retSig: "CallContext",
              desc: "Return the authenticated CallContext for the currently executing MCP tool call (contains caller, session_id, deadline_unix_ms, raise_if_cancelled(), and report_progress()).",
              params: [],
              returns: "Active CallContext for the in-flight tool invocation.",
              rtype: "CallContext",
              raises: "RuntimeError – If called outside an active @app.tool handler.",
              example: "from yuos_sdk.mcp import current_context\n\nctx = current_context()\nctx.raise_if_cancelled()"
            }
          ]
        },
        {
          id: "mod-files",
          name: "yuos_sdk.files",
          summary: "Brokered single-file binary snapshots (read_file) and atomic compare-and-replace (replace_file) for MCP tools.",
          items: [
            {
              id: "files-read-file",
              qual: "yuos_sdk.files.",
              name: "read_file",
              paramsSig: "(path, *, binary, deadline_unix_ms, check_cancelled=None)",
              retSig: "FileSnapshot",
              desc: "Read a complete binary snapshot (up to 32 MiB) of an existing file authorized with transform: \"brokered-file\" in an MCP tool.",
              params: [
                ["path", "str", "Absolute literal file path bound to the MCP tool argument."],
                ["binary", "str", "Path to the installed yu binary (e.g. \"/usr/local/bin/yu\")."],
                ["deadline_unix_ms", "int", "Call deadline from current_context().deadline_unix_ms."],
                ["check_cancelled", "Callable[[], None] | None, optional", "Cancellation callback from current_context().raise_if_cancelled."]
              ],
              returns: "FileSnapshot(content: bytes, revision: str).",
              rtype: "FileSnapshot",
              raises: "ValueError, KernelDenied, KernelUnavailable",
              example: "from yuos_sdk.files import read_file, replace_file\nfrom yuos_sdk.mcp import current_context\n\nctx = current_context()\nsnap = read_file(path, binary=\"/usr/local/bin/yu\", deadline_unix_ms=ctx.deadline_unix_ms)"
            },
            {
              id: "files-replace-file",
              qual: "yuos_sdk.files.",
              name: "replace_file",
              paramsSig: "(path, revision, content, *, binary, deadline_unix_ms, check_cancelled=None)",
              retSig: "None",
              desc: "Atomically replace the file read earlier in the same invocation if its on-disk state still matches revision.",
              params: [
                ["path", "str", "Same absolute literal path passed to read_file()."],
                ["revision", "str", "Opaque 32-hex-char revision token from FileSnapshot.revision."],
                ["content", "bytes", "Replacement bytes (max 32 MiB)."],
                ["binary", "str", "Path to /usr/local/bin/yu."],
                ["deadline_unix_ms", "int", "Call deadline in milliseconds."],
                ["check_cancelled", "Callable[[], None] | None, optional", "Cancellation callback."]
              ],
              returns: "None",
              rtype: "None",
              raises: "ValueError, KernelDenied, KernelUnavailable",
              example: "replace_file(path, snap.revision, updated_bytes, binary=\"/usr/local/bin/yu\", deadline_unix_ms=ctx.deadline_unix_ms)"
            }
          ]
        }
      ]
    },
    node: {
      title: "@yuos/sdk (Node.js / TypeScript API Reference)",
      install: "npm install @yuos/sdk",
      lead: "Official Node.js & TypeScript SDK for Yu OS Apps (Node 18+). Exports app, ai, gui, mcp, files, events, and objects namespaces.",
      modules: [
        {
          id: "mod-app",
          name: "app (@yuos/sdk)",
          summary: "Application entrypoints (app.command, app.desktop, app.run) and OS helpers (dataDir, notify, clipboard, secret, permissions).",
          items: [
            {
              id: "app-command",
              qual: "app.",
              name: "command",
              paramsSig: "(nameOrHandler, maybeHandler?)",
              retSig: "void",
              desc: "Register a CLI command handler for yu app <id> <name> [args...]. Pass a named function app.command(async function summarize(args) { ... }) or an explicit name app.command(\"summarize\", fn).",
              params: [
                ["nameOrHandler", "string | ((args: string[]) => unknown)", "Operation name in app.json or a named handler function."],
                ["maybeHandler", "(args: string[]) => unknown, optional", "Handler function when the first argument is a string name."]
              ],
              returns: "void (the handler's resolved return value is printed as JSON to stdout when app.run() executes).",
              rtype: "void",
              raises: "Error – If the command name is empty or registered twice.",
              example: "import { app } from \"@yuos/sdk\";\n\napp.command(async function summarize(args: string[]) {\n  return { summary: args[0] };\n});"
            },
            {
              id: "app-desktop",
              qual: "app.",
              name: "desktop",
              paramsSig: "(handler)",
              retSig: "void",
              desc: "Register the desktop GUI entry function invoked when launched from the desktop or via yu app <id> --gui.",
              params: [
                ["handler", "() => unknown | Promise<unknown>", "Zero-argument function that launches your desktop window."]
              ],
              returns: "void",
              rtype: "void",
              raises: "Error – If a desktop handler is already registered.",
              example: "app.desktop(async () => {\n  // Start your Electron / Tauri / GTK window\n});"
            },
            {
              id: "app-run",
              qual: "app.",
              name: "run",
              paramsSig: "()",
              retSig: "Promise<void>",
              desc: "Dispatch the current process invocation to the registered command or desktop handler.",
              params: [],
              returns: "Promise<void>",
              rtype: "Promise<void>",
              raises: "Error – If no matching handler is registered.",
              example: "await app.run();"
            },
            {
              id: "app-data-dir",
              qual: "app.",
              name: "dataDir",
              paramsSig: "()",
              retSig: "string",
              desc: "Return (and create if needed) this App's isolated private data directory.",
              params: [],
              returns: "Absolute directory path string.",
              rtype: "string",
              raises: "Error – If the directory cannot be created.",
              example: "const dir = app.dataDir();"
            },
            {
              id: "app-notify",
              qual: "app.",
              name: "notify",
              paramsSig: "(summary, body = \"\")",
              retSig: "Record<string, unknown>",
              desc: "Post a system notification (requires ui.notify in app.json).",
              params: [
                ["summary", "string", "Notification title (non-empty)."],
                ["body", "string, optional", "Notification body text. Default: \"\"."]
              ],
              returns: "Broker response object.",
              rtype: "Record<string, unknown>",
              raises: "Error – If summary is empty or permission is denied.",
              example: "app.notify(\"Task complete\", \"Report exported.\");"
            },
            {
              id: "app-clipboard-read",
              qual: "app.",
              name: "clipboardRead",
              paramsSig: "()",
              retSig: "string",
              desc: "Read UTF-8 text from the system clipboard (clipboard.read).",
              params: [],
              returns: "Clipboard text.",
              rtype: "string",
              raises: "Error – If clipboard.read is denied.",
              example: "const text = app.clipboardRead();"
            },
            {
              id: "app-clipboard-write",
              qual: "app.",
              name: "clipboardWrite",
              paramsSig: "(text)",
              retSig: "void",
              desc: "Write UTF-8 text to the system clipboard (clipboard.write).",
              params: [
                ["text", "string", "Text to copy to the clipboard."]
              ],
              returns: "void",
              rtype: "void",
              raises: "Error – If clipboard.write is denied.",
              example: "app.clipboardWrite(\"Hello from Node SDK\");"
            },
            {
              id: "app-secret",
              qual: "app.",
              name: "secret",
              paramsSig: "(name, namespace = \"default\")",
              retSig: "string",
              desc: "Load a secret from the Yu OS Credential Store (secret.use).",
              params: [
                ["name", "string", "Credential name."],
                ["namespace", "string, optional", "Credential namespace. Default: \"default\"."]
              ],
              returns: "Secret string value.",
              rtype: "string",
              raises: "Error – If the secret is missing or unauthorized.",
              example: "const apiKey = app.secret(\"BRAVE_SEARCH_API_KEY\");"
            },
            {
              id: "app-require-permission",
              qual: "app.",
              name: "requirePermission",
              paramsSig: "(verb, scope = {})",
              retSig: "void",
              desc: "Require that the current session holds verb for a dynamic scope ({ path?, host?, name?, selfRef?, wild? }). Use app.checkPermission(verb, scope) to inspect the decision object without throwing.",
              params: [
                ["verb", "string", "Capability verb (e.g. \"net.dial\")."],
                ["scope", "PermissionScope, optional", "Object with at most one of path, host, name, selfRef, or wild."]
              ],
              returns: "void",
              rtype: "void",
              raises: "Error – If the capability check is denied.",
              example: "app.requirePermission(\"net.dial\", { host: \"api.example.com:443\" });"
            }
          ]
        },
        {
          id: "mod-ai",
          name: "ai (@yuos/sdk)",
          summary: "Async system AI functions (chat, chatStream, embed, tts, stt, imagegen, decide, budget).",
          items: [
            {
              id: "ai-chat",
              qual: "ai.",
              name: "chat",
              paramsSig: "(prompt, opts = {})",
              retSig: "Promise<AiResponse>",
              desc: "Call the user's configured chat model (\"needs\": [\"ai.chat\"]).",
              params: [
                ["prompt", "string", "Non-empty prompt text."],
                ["opts.system", "string, optional", "Optional system instruction."],
                ["opts.images", "Uint8Array[], optional", "Up to 4 PNG, JPEG, or WebP byte arrays (auto-detected)."],
                ["opts.maxUnits", "number | bigint, optional", "Per-call budget ceiling."],
                ["opts.functions", "AiAppTool[], optional", "App-owned tool definitions."],
                ["opts.toolTurns", "AiToolTurn[], optional", "Prior tool turns."],
                ["opts.signal", "AbortSignal, optional", "Optional cancellation signal."]
              ],
              returns: "Promise<AiResponse> with .provider, .model, .units, and .output ({ text, toolCalls }).",
              rtype: "Promise<AiResponse>",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiSafetyViolation, AiUnavailable",
              example: "import { ai } from \"@yuos/sdk\";\n\nconst reply = await ai.chat(\"Summarize this.\", { system: \"One sentence.\" });\nconsole.log(reply.output.text);"
            },
            {
              id: "ai-chat-stream",
              qual: "ai.",
              name: "chatStream",
              paramsSig: "(prompt, opts = {})",
              retSig: "AsyncGenerator<string, void, unknown>",
              desc: "Stream text chunks from the chat model as an async generator.",
              params: [
                ["prompt", "string", "Non-empty prompt text."],
                ["opts", "ChatOptions, optional", "Same options as ai.chat (system, images, maxUnits, signal)."]
              ],
              returns: "AsyncGenerator yielding string chunks.",
              rtype: "AsyncGenerator<string>",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiSafetyViolation, AiUnavailable",
              example: "for await (const chunk of ai.chatStream(\"Write a poem.\")) {\n  process.stdout.write(chunk);\n}"
            },
            {
              id: "ai-embed",
              qual: "ai.",
              name: "embed",
              paramsSig: "(text, opts = {})",
              retSig: "Promise<ModelResponse<EmbeddingOutput>>",
              desc: "Compute an embedding vector (\"needs\": [\"ai.embed\"]).",
              params: [
                ["text", "string", "Text to embed."],
                ["opts", "ModelOptions, optional", "Optional { maxUnits, signal }."]
              ],
              returns: "Promise resolving to { provider, model, units, output: { vector: number[] } }.",
              rtype: "Promise<ModelResponse<EmbeddingOutput>>",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiUnavailable",
              example: "const { output } = await ai.embed(\"search query\");\nconsole.log(output.vector.length);"
            },
            {
              id: "ai-tts",
              qual: "ai.",
              name: "tts",
              paramsSig: "(text, opts = {})",
              retSig: "Promise<ModelResponse<SpeechOutput>>",
              desc: "Synthesize speech audio bytes from text (\"needs\": [\"ai.audio.tts\"]).",
              params: [
                ["text", "string", "Text to speak."],
                ["opts", "ModelOptions, optional", "Optional { maxUnits, signal }."]
              ],
              returns: "Promise resolving to { output: { format: string, audio: Uint8Array, sample_rate_hz?: number } }.",
              rtype: "Promise<ModelResponse<SpeechOutput>>",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiUnavailable",
              example: "const speech = (await ai.tts(\"Hello world\")).output;\nfs.writeFileSync(\"hello.wav\", speech.audio);"
            },
            {
              id: "ai-stt",
              qual: "ai.",
              name: "stt",
              paramsSig: "(audio, opts = {})",
              retSig: "Promise<ModelResponse<TranscriptionOutput>>",
              desc: "Transcribe WAV, MP3, or Ogg audio bytes to text (\"needs\": [\"ai.audio.stt\"]).",
              params: [
                ["audio", "Uint8Array", "Raw audio bytes (WAV, MP3, or Ogg)."],
                ["opts", "ModelOptions, optional", "Optional { maxUnits, signal }."]
              ],
              returns: "Promise resolving to { output: { text: string } }.",
              rtype: "Promise<ModelResponse<TranscriptionOutput>>",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiUnavailable",
              example: "const res = await ai.stt(fs.readFileSync(\"memo.wav\"));\nconsole.log(res.output.text);"
            },
            {
              id: "ai-imagegen",
              qual: "ai.",
              name: "imagegen",
              paramsSig: "(prompt, opts = {})",
              retSig: "Promise<ModelResponse<ImageOutput>>",
              desc: "Generate an image from a text prompt (\"needs\": [\"ai.image.generate\"]).",
              params: [
                ["prompt", "string", "Image description."],
                ["opts", "ModelOptions, optional", "Optional { maxUnits, signal }."]
              ],
              returns: "Promise resolving to { output: { format: string, image: Uint8Array } }.",
              rtype: "Promise<ModelResponse<ImageOutput>>",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiSafetyViolation, AiUnavailable",
              example: "const img = (await ai.imagegen(\"Sunset over ocean\")).output;\nfs.writeFileSync(`out.${img.format}`, img.image);"
            },
            {
              id: "ai-decide",
              qual: "ai.",
              name: "decide",
              paramsSig: "(state, questions, opts = {})",
              retSig: "Promise<ModelResponse<DecisionOutput>>",
              desc: "Evaluate structured state and answer decision questions (\"needs\": [\"ai.decide\"]).",
              params: [
                ["state", "WireJsonValue", "Structured state object."],
                ["questions", "Record<string, WireJsonValue>", "Questions map."],
                ["opts", "ModelOptions, optional", "Optional { maxUnits, signal }."]
              ],
              returns: "Promise resolving to { output: { answers: Record<string, WireJsonValue> } }.",
              rtype: "Promise<ModelResponse<DecisionOutput>>",
              raises: "AiDenied, AiBudgetExceeded, AiInvalidArgs, AiUnavailable",
              example: "const res = await ai.decide({ battery: 10 }, { lowPower: \"Enable low-power mode?\" });"
            },
            {
              id: "ai-budget",
              qual: "ai.",
              name: "budget",
              paramsSig: "(opts = {})",
              retSig: "Promise<BudgetUsage>",
              desc: "Query this App's monthly AI unit usage.",
              params: [
                ["opts.appId", "string, optional", "App ID override."],
                ["opts.signal", "AbortSignal, optional", "Cancellation signal."]
              ],
              returns: "Promise<BudgetUsage> ({ app, period, unitsUsed }).",
              rtype: "Promise<BudgetUsage>",
              raises: "AiUnavailable, AiError",
              example: "const b = await ai.budget();\nconsole.log(b.unitsUsed);"
            }
          ]
        },
        {
          id: "mod-gui",
          name: "gui (@yuos/sdk)",
          summary: "Native file/folder dialogs (openFile, saveFile, selectFolder), window context, and Agent overlay.",
          items: [
            {
              id: "gui-open-file",
              qual: "gui.",
              name: "openFile",
              paramsSig: "(title = \"Open file\")",
              retSig: "string | null",
              desc: "Open the native file picker dialog and grant read access to the selected file.",
              params: [
                ["title", "string, optional", "Dialog title. Default: \"Open file\"."]
              ],
              returns: "Selected file path, or null if cancelled.",
              rtype: "string | null",
              raises: "Error – If not in a GUI session.",
              example: "const path = gui.openFile(\"Open Document\");"
            },
            {
              id: "gui-save-file",
              qual: "gui.",
              name: "saveFile",
              paramsSig: "(defaultName = \"untitled.txt\", title = \"Save file\")",
              retSig: "string | null",
              desc: "Open the native Save dialog (ui.prompt).",
              params: [
                ["defaultName", "string, optional", "Suggested filename."],
                ["title", "string, optional", "Dialog title."]
              ],
              returns: "Chosen file path, or null if cancelled.",
              rtype: "string | null",
              raises: "Error – If ui.prompt is denied.",
              example: "const outPath = gui.saveFile(\"notes.md\");"
            },
            {
              id: "gui-select-folder",
              qual: "gui.",
              name: "selectFolder",
              paramsSig: "(title = \"Select folder\")",
              retSig: "string | null",
              desc: "Open the native folder picker dialog (ui.prompt).",
              params: [
                ["title", "string, optional", "Dialog title."]
              ],
              returns: "Chosen directory path, or null if cancelled.",
              rtype: "string | null",
              raises: "Error – If ui.prompt is denied.",
              example: "const dir = gui.selectFolder();"
            },
            {
              id: "gui-context",
              qual: "gui.",
              name: "context",
              paramsSig: "(files?)",
              retSig: "GuiContext",
              desc: "Read the desktop launch context ({ appId, files }) and openAgentOverlay(hint).",
              params: [
                ["files", "readonly string[], optional", "Optional explicit file list."]
              ],
              returns: "GuiContext ({ appId: string, files: readonly string[] }).",
              rtype: "GuiContext",
              raises: "GuiLaunchError – If YUOS_APP_ID is unset.",
              example: "const ctx = gui.context();\nawait gui.openAgentOverlay(\"Summarize this window\");"
            }
          ]
        },
        {
          id: "mod-mcp",
          name: "mcp (@yuos/sdk)",
          summary: "Manifest-bound MCP server runtime for the Yu OS System Agent.",
          items: [
            {
              id: "mcp-app",
              qual: "mcp.App.",
              name: "fromManifest",
              paramsSig: "(manifestPath?)",
              retSig: "mcp.App",
              desc: "Load app.json, bind tools with app.tool(name, fn), and serve stdio with await app.serve().",
              params: [
                ["manifestPath", "string, optional", "Path to app.json."]
              ],
              returns: "mcp.App instance.",
              rtype: "mcp.App",
              raises: "ManifestError – If app.json is invalid.",
              example: "import { mcp } from \"@yuos/sdk\";\n\nconst server = mcp.App.fromManifest();\nserver.tool(\"notes.add\", async ({ title }) => ({ saved: title }));\nawait server.serve();"
            }
          ]
        },
        {
          id: "mod-files",
          name: "files (@yuos/sdk)",
          summary: "Brokered single-file binary snapshots (readFile, replaceFile).",
          items: [
            {
              id: "files-read-file",
              qual: "files.",
              name: "readFile",
              paramsSig: "(path, options = {})",
              retSig: "FileSnapshot",
              desc: "Read a complete binary file snapshot (up to 32 MiB) in a brokered-file MCP tool.",
              params: [
                ["path", "string", "Absolute literal file path."],
                ["options", "FileOptions, optional", "{ deadlineUnixMs?, checkCancelled? }."]
              ],
              returns: "FileSnapshot ({ content: Buffer, revision: string }).",
              rtype: "FileSnapshot",
              raises: "Error – On invalid path or broker denial.",
              example: "const snap = files.readFile(path, { deadlineUnixMs: ctx.deadlineUnixMs });"
            },
            {
              id: "files-replace-file",
              qual: "files.",
              name: "replaceFile",
              paramsSig: "(path, revision, content, options = {})",
              retSig: "void",
              desc: "Atomically replace the file read earlier in the same invocation if revision still matches.",
              params: [
                ["path", "string", "Target file path."],
                ["revision", "string", "Revision token from readFile()."],
                ["content", "Buffer | Uint8Array", "Replacement bytes (max 32 MiB)."],
                ["options", "FileOptions, optional", "{ deadlineUnixMs?, checkCancelled? }."]
              ],
              returns: "void",
              rtype: "void",
              raises: "Error – On revision conflict or broker failure.",
              example: "files.replaceFile(path, snap.revision, updatedBuffer);"
            }
          ]
        }
      ]
    },
    go: {
      title: "yuossdk (Go API Reference)",
      install: "go get github.com/xiaoyu-work/Yu-OS/yuos-sdk/go@v0.1.0",
      lead: "Official Go SDK for Yu OS Apps (Go 1.21+). Package import path: github.com/xiaoyu-work/Yu-OS/yuos-sdk/go (package yuossdk).",
      modules: [
        {
          id: "mod-app",
          name: "App & System Helpers (yuossdk)",
          summary: "HandleCommand, HandleDesktop, RunApp, DataDir, Notify, Clipboard, Secret, and RequirePermission.",
          items: [
            {
              id: "app-command",
              qual: "yuossdk.",
              name: "HandleCommand",
              paramsSig: "(name string, fn CommandFunc)",
              retSig: "",
              desc: "Register fn (func(args []string) (any, error)) for yu app <id> <name>.",
              params: [
                ["name", "string", "Operation name declared under operations in app.json."],
                ["fn", "func(args []string) (any, error)", "Command handler function."]
              ],
              returns: "None (panics if name is empty or registered twice).",
              rtype: "void",
              raises: "Returned error from fn is written to stderr and exits non-zero.",
              example: "yuossdk.HandleCommand(\"summarize\", func(args []string) (any, error) {\n\treturn map[string]any{\"summary\": args[0]}, nil\n})"
            },
            {
              id: "app-desktop",
              qual: "yuossdk.",
              name: "HandleDesktop",
              paramsSig: "(fn DesktopFunc)",
              retSig: "",
              desc: "Register fn (func() error) to open the App's desktop window when launched with --gui.",
              params: [
                ["fn", "func() error", "Zero-argument desktop entry function."]
              ],
              returns: "None",
              rtype: "void",
              raises: "Returned error from fn.",
              example: "yuossdk.HandleDesktop(func() error {\n\treturn openMainWindow()\n})"
            },
            {
              id: "app-run",
              qual: "yuossdk.",
              name: "RunApp",
              paramsSig: "()",
              retSig: "error",
              desc: "Dispatch the current process invocation to the registered command or desktop handler.",
              params: [],
              returns: "error if dispatch or handler execution fails.",
              rtype: "error",
              raises: "error",
              example: "if err := yuossdk.RunApp(); err != nil {\n\tlog.Fatal(err)\n}"
            },
            {
              id: "app-data-dir",
              qual: "yuossdk.",
              name: "DataDir",
              paramsSig: "()",
              retSig: "(string, error)",
              desc: "Return (and create if needed) this App's isolated private data directory.",
              params: [],
              returns: "(string, error) – Directory path.",
              rtype: "(string, error)",
              raises: "error",
              example: "dir, err := yuossdk.DataDir()"
            },
            {
              id: "app-notify",
              qual: "yuossdk.",
              name: "Notify",
              paramsSig: "(summary, body string)",
              retSig: "error",
              desc: "Post a system notification (requires ui.notify in app.json).",
              params: [
                ["summary", "string", "Notification title (non-empty)."],
                ["body", "string", "Notification body."]
              ],
              returns: "error",
              rtype: "error",
              raises: "error",
              example: "_ = yuossdk.Notify(\"Done\", \"Export finished.\")"
            },
            {
              id: "app-clipboard",
              qual: "yuossdk.",
              name: "ClipboardRead / ClipboardWrite",
              paramsSig: "() (string, error) / (text string) error",
              retSig: "(string, error) / error",
              desc: "Read or write UTF-8 text on the system clipboard (clipboard.read / clipboard.write).",
              params: [
                ["text", "string", "Text to write (for ClipboardWrite)."]
              ],
              returns: "Clipboard text or error.",
              rtype: "(string, error) / error",
              raises: "error",
              example: "text, err := yuossdk.ClipboardRead()\n_ = yuossdk.ClipboardWrite(\"copied\")"
            },
            {
              id: "app-secret",
              qual: "yuossdk.",
              name: "Secret",
              paramsSig: "(name, namespace string)",
              retSig: "(string, error)",
              desc: "Load an encrypted secret from the Yu OS Credential Store (secret.use). Pass \"\" or \"default\" for the default namespace.",
              params: [
                ["name", "string", "Credential name."],
                ["namespace", "string", "Credential namespace (defaults to \"default\" when empty)."]
              ],
              returns: "Decrypted secret value.",
              rtype: "(string, error)",
              raises: "error",
              example: "token, err := yuossdk.Secret(\"GOOGLE_ACCESS_TOKEN\", \"default\")"
            },
            {
              id: "app-require-permission",
              qual: "yuossdk.",
              name: "RequirePermission",
              paramsSig: "(verb string, scope PermissionScope)",
              retSig: "error",
              desc: "Require that the current session holds verb for scope (PermissionScope{Path, Host, Name, SelfRef, Wild}). Use CheckPermission to inspect the raw decision map.",
              params: [
                ["verb", "string", "Capability verb."],
                ["scope", "PermissionScope", "Dynamic scope struct."]
              ],
              returns: "nil if allowed, non-nil error if denied.",
              rtype: "error",
              raises: "error",
              example: "err := yuossdk.RequirePermission(\"net.dial\", yuossdk.PermissionScope{Host: \"example.com:443\"})"
            }
          ]
        },
        {
          id: "mod-ai",
          name: "AI Models (yuossdk)",
          summary: "Chat, ChatStream, Embed, TTS, STT, ImageGen, Decide, and Budget.",
          items: [
            {
              id: "ai-chat",
              qual: "yuossdk.",
              name: "Chat",
              paramsSig: "(prompt string, opts ChatOptions)",
              retSig: "(*AiResponse, error)",
              desc: "Call the user's configured chat model (\"needs\": [\"ai.chat\"]). Use ChatContext(ctx, prompt, opts) for context cancellation.",
              params: [
                ["prompt", "string", "Non-empty prompt text."],
                ["opts.System", "string", "Optional system instruction."],
                ["opts.Images", "[][]byte", "Up to 4 PNG, JPEG, or WebP byte slices."],
                ["opts.MaxUnits", "int", "Optional per-call unit ceiling."],
                ["opts.Functions", "[]AiAppTool", "Optional App tool schemas."],
                ["opts.ToolTurns", "[]AiToolTurn", "Prior tool turns."]
              ],
              returns: "*AiResponse with .Provider, .Model, .Units, and .Output (.Text, .ToolCalls).",
              rtype: "(*AiResponse, error)",
              raises: "*AiDeniedError, *AiBudgetExceededError, *AiInvalidArgsError, *AiSafetyViolationError, *AiUnavailableError",
              example: "reply, err := yuossdk.Chat(\"Summarize this.\", yuossdk.ChatOptions{System: \"Concise.\"})\nfmt.Println(reply.Output.Text)"
            },
            {
              id: "ai-chat-stream",
              qual: "yuossdk.",
              name: "ChatStream",
              paramsSig: "(ctx context.Context, prompt string, opts ChatOptions, onChunk func(string) error)",
              retSig: "(*AiResponse, error)",
              desc: "Stream text chunks to onChunk from the configured chat model.",
              params: [
                ["ctx", "context.Context", "Request context."],
                ["prompt", "string", "Non-empty prompt text."],
                ["opts", "ChatOptions", "Chat options."],
                ["onChunk", "func(chunk string) error", "Callback invoked for each chunk."]
              ],
              returns: "(*AiResponse, error)",
              rtype: "(*AiResponse, error)",
              raises: "error",
              example: "_, err := yuossdk.ChatStream(ctx, \"Hello\", yuossdk.ChatOptions{}, func(c string) error {\n\tfmt.Print(c)\n\treturn nil\n})"
            },
            {
              id: "ai-other",
              qual: "yuossdk.",
              name: "Embed / TTS / STT / ImageGen / Decide",
              paramsSig: "(ctx context.Context, input ..., opts ModelOptions)",
              retSig: "(*ModelResponse[T], error)",
              desc: "Invoke the configured embedding, speech synthesis, speech transcription, image generation, or decision model.",
              params: [
                ["Embed(ctx, text, opts)", "string -> *ModelResponse[Embedding]", "Returns .Output.Vector ([]float64)."],
                ["TTS(ctx, text, opts)", "string -> *ModelResponse[Speech]", "Returns .Output.Audio ([]byte) and .Output.Format (string)."],
                ["STT(ctx, audioBytes, opts)", "[]byte -> *ModelResponse[Transcription]", "Accepts WAV/MP3/Ogg bytes; returns .Output.Text (string)."],
                ["ImageGen(ctx, prompt, opts)", "string -> *ModelResponse[Image]", "Returns .Output.Image ([]byte) and .Output.Format (string)."],
                ["Decide(ctx, state, questions, opts)", "any, map[string]any -> *ModelResponse[Decision]", "Returns .Output.Answers (map[string]any)."]
              ],
              returns: "(*ModelResponse[T], error)",
              rtype: "(*ModelResponse[T], error)",
              raises: "error",
              example: "emb, err := yuossdk.Embed(ctx, \"query\", yuossdk.ModelOptions{})\nspeech, err := yuossdk.TTS(ctx, \"Hello\", yuossdk.ModelOptions{})"
            }
          ]
        },
        {
          id: "mod-gui",
          name: "GUI & Files (yuossdk)",
          summary: "OpenFile, SaveFile, SelectFolder, Context, ReadFile, and ReplaceFile.",
          items: [
            {
              id: "gui-dialogs",
              qual: "yuossdk.",
              name: "OpenFile / SaveFile / SelectFolder",
              paramsSig: "(title string) (string, error)",
              retSig: "(string, error)",
              desc: "Open native system file/folder dialogs. OpenFile(title) grants read access to the chosen file automatically; SaveFile(defaultName, title) and SelectFolder(title) use ui.prompt.",
              params: [
                ["defaultName", "string", "Suggested filename (for SaveFile)."],
                ["title", "string", "Dialog window title."]
              ],
              returns: "Chosen path string (or \"\" if cancelled) and error.",
              rtype: "(string, error)",
              raises: "error",
              example: "path, err := yuossdk.OpenFile(\"Select file\")"
            },
            {
              id: "files-go",
              qual: "yuossdk.",
              name: "ReadFile / ReplaceFile",
              paramsSig: "(path string) (*FileSnapshot, error) / (path, revision string, content []byte) error",
              retSig: "(*FileSnapshot, error) / error",
              desc: "Read and atomically replace a brokered file (up to 32 MiB) in an MCP tool.",
              params: [
                ["path", "string", "Absolute literal file path."],
                ["revision", "string", "Opaque revision token from FileSnapshot.Revision."],
                ["content", "[]byte", "Replacement bytes."]
              ],
              returns: "*FileSnapshot ({ Content: []byte, Revision: string }) / error.",
              rtype: "(*FileSnapshot, error) / error",
              raises: "error",
              example: "snap, err := yuossdk.ReadFile(path)\nerr = yuossdk.ReplaceFile(path, snap.Revision, updatedBytes)"
            }
          ]
        }
      ]
    },
    rust: {
      title: "yuos_sdk (Rust Crate API Reference)",
      install: "cargo add yuos-sdk",
      lead: "Official Rust SDK for Yu OS Apps (Edition 2021). Modules: yuos_sdk::app, yuos_sdk::ai, yuos_sdk::gui, yuos_sdk::mcp, yuos_sdk::files, yuos_sdk::events, yuos_sdk::objects.",
      modules: [
        {
          id: "mod-app",
          name: "yuos_sdk::app",
          summary: "App builder (App::new, command, desktop, run) and OS helpers (data_dir, notify, clipboard_read/write, secret, require_permission).",
          items: [
            {
              id: "app-builder",
              qual: "yuos_sdk::app::",
              name: "App",
              paramsSig: "::new().command(name, fn).desktop(fn).run()",
              retSig: "Result<(), AppError>",
              desc: "Builder that registers CLI command handlers and the zero-argument desktop handler, then dispatches the current launch.",
              params: [
                ["name", "&str", "Operation name in app.json."],
                ["command fn", "FnOnce(Vec<String>) -> Result<serde_json::Value, FunctionError>", "Receives validated CLI arguments and returns JSON."],
                ["desktop fn", "FnOnce() -> Result<(), FunctionError>", "Zero-argument desktop entry function."]
              ],
              returns: "Result<(), AppError>",
              rtype: "Result<(), AppError>",
              raises: "AppError",
              example: "use yuos_sdk::app::{App, AppError};\n\nfn main() -> Result<(), AppError> {\n    App::new()\n        .command(\"summarize\", |args| Ok(serde_json::json!({ \"summary\": args[0] })))\n        .desktop(|| Ok(()))\n        .run()\n}"
            },
            {
              id: "app-helpers-rs",
              qual: "yuos_sdk::app::",
              name: "data_dir / notify / clipboard_read / clipboard_write / secret / require_permission",
              paramsSig: "",
              retSig: "Result<T, BridgeError>",
              desc: "System helper functions in yuos_sdk::app.",
              params: [
                ["data_dir()", "-> std::io::Result<PathBuf>", "Return (and create) this App's private data directory."],
                ["notify(summary: &str, body: &str)", "-> Result<Value, BridgeError>", "Post a desktop notification (ui.notify)."],
                ["clipboard_read()", "-> Result<String, BridgeError>", "Read UTF-8 text from the clipboard (clipboard.read)."],
                ["clipboard_write(text: &str)", "-> Result<(), BridgeError>", "Write UTF-8 text to the clipboard (clipboard.write)."],
                ["secret(name: &str, namespace: &str)", "-> Result<String, BridgeError>", "Load a secret from the Yu OS Credential Store (secret.use)."],
                ["require_permission(verb: &str, scope: PermissionScope)", "-> Result<(), BridgeError>", "Check a dynamic capability scope (Path, Host, Name, SelfRef, Wild, Unscoped)."]
              ],
              returns: "Result<T, BridgeError>",
              rtype: "Result<T, BridgeError>",
              raises: "BridgeError",
              example: "use yuos_sdk::app::{self, PermissionScope};\n\nlet dir = app::data_dir()?;\nlet token = app::secret(\"API_KEY\", \"default\")?;\napp::require_permission(\"net.dial\", PermissionScope::host(\"api.example.com:443\"))?;"
            }
          ]
        },
        {
          id: "mod-ai",
          name: "yuos_sdk::ai",
          summary: "chat, chat_stream, embed, tts, stt, imagegen, decide, and budget.",
          items: [
            {
              id: "ai-chat-rs",
              qual: "yuos_sdk::ai::",
              name: "chat / chat_stream",
              paramsSig: "(prompt: &str, opts: ChatOpts)",
              retSig: "Result<AiResponse, AiError>",
              desc: "Call or stream from the owner's configured chat model (\"needs\": [\"ai.chat\"]). Configure options with ChatOpts::new().system(...).images(...).max_units(...).functions(...).",
              params: [
                ["prompt", "&str", "Non-empty prompt text."],
                ["opts", "ChatOpts", "Builder with .system(str), .images(Vec<Vec<u8>>), .max_units(u64), .functions(Vec<AiAppTool>), .tool_turns(Vec<AiToolTurn>)."],
                ["on_chunk (for chat_stream)", "FnMut(&str)", "Callback receiving streamed text chunks."]
              ],
              returns: "Result<AiResponse, AiError> (.output.text: String, .output.tool_calls: Vec<ProposedToolCall>).",
              rtype: "Result<AiResponse, AiError>",
              raises: "AiError::{Denied, BudgetExceeded, SafetyViolation, InvalidArg, Unavailable}",
              example: "use yuos_sdk::ai::{self, ChatOpts};\n\nlet reply = ai::chat(\"Summarize this.\", ChatOpts::new().system(\"One sentence.\"))?;\nprintln!(\"{}\", reply.output.text);"
            },
            {
              id: "ai-slots-rs",
              qual: "yuos_sdk::ai::",
              name: "embed / tts / stt / imagegen / decide",
              paramsSig: "(input, opts: ModelOpts)",
              retSig: "Result<ModelResponse<T>, AiError>",
              desc: "Typed calls for the owner's configured embedding, speech synthesis, speech transcription, image generation, and decision models.",
              params: [
                ["embed(text: &str, opts: ModelOpts)", "-> Result<ModelResponse<Embedding>, AiError>", "Returns .output.vector: Vec<f64>."],
                ["tts(text: &str, opts: ModelOpts)", "-> Result<ModelResponse<Speech>, AiError>", "Returns .output.audio: Vec<u8> and .output.format: String."],
                ["stt(audio: &[u8], opts: ModelOpts)", "-> Result<ModelResponse<Transcription>, AiError>", "Accepts WAV/MP3/Ogg bytes; returns .output.text: String."],
                ["imagegen(prompt: &str, opts: ModelOpts)", "-> Result<ModelResponse<Image>, AiError>", "Returns .output.image: Vec<u8> and .output.format: String."],
                ["decide(state: Value, questions: Map<String, Value>, opts: ModelOpts)", "-> Result<ModelResponse<Decision>, AiError>", "Returns .output.answers: Map<String, Value>."]
              ],
              returns: "Result<ModelResponse<T>, AiError>",
              rtype: "Result<ModelResponse<T>, AiError>",
              raises: "AiError",
              example: "use yuos_sdk::ai::{self, ModelOpts};\n\nlet emb = ai::embed(\"hello\", ModelOpts::default())?;\nlet speech = ai::tts(\"hello\", ModelOpts::default())?;"
            }
          ]
        },
        {
          id: "mod-gui",
          name: "yuos_sdk::gui & yuos_sdk::files",
          summary: "open_file, save_file, select_folder, Context::from_env, read_file, and replace_file.",
          items: [
            {
              id: "gui-rs",
              qual: "yuos_sdk::gui::",
              name: "open_file / save_file / select_folder / Context",
              paramsSig: "",
              retSig: "Result<Option<String>, String>",
              desc: "System file/folder pickers and GUI launch context.",
              params: [
                ["open_file(title: &str)", "-> Result<Option<String>, String>", "Open file picker; automatically grants read access to the chosen file."],
                ["save_file(default_name: &str, title: &str)", "-> Result<Option<String>, String>", "Open Save dialog (ui.prompt)."],
                ["select_folder(title: &str)", "-> Result<Option<String>, String>", "Open folder picker (ui.prompt)."],
                ["Context::from_env()", "-> Result<Context, ContextError>", "Read app_id, files, and call .open_agent_overlay(hint)."]
              ],
              returns: "Result<Option<String>, String>",
              rtype: "Result<Option<String>, String>",
              raises: "String / ContextError",
              example: "if let Some(path) = yuos_sdk::gui::open_file(\"Open Document\")? {\n    let data = std::fs::read(path)?;\n}"
            },
            {
              id: "files-rs",
              qual: "yuos_sdk::files::",
              name: "read_file / replace_file",
              paramsSig: "(path: &str, ...)",
              retSig: "Result<FileSnapshot, BridgeError>",
              desc: "Brokered single-file binary snapshot and atomic replacement for MCP tools.",
              params: [
                ["read_file(path: &str)", "-> Result<FileSnapshot, BridgeError>", "Returns FileSnapshot { content: Vec<u8>, revision: String }."],
                ["replace_file(path: &str, revision: &str, content: &[u8])", "-> Result<(), BridgeError>", "Atomically replace the file if revision matches."]
              ],
              returns: "Result<FileSnapshot / (), BridgeError>",
              rtype: "Result<..., BridgeError>",
              raises: "BridgeError",
              example: "let snap = yuos_sdk::files::read_file(&path)?;\nyuos_sdk::files::replace_file(&path, &snap.revision, &new_bytes)?;"
            }
          ]
        }
      ]
    }
  };

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function renderApiPage(lang) {
    var data = API_DATA[lang] || API_DATA.python;
    var sidebarEl = document.getElementById("api-sidebar");
    var contentEl = document.getElementById("api-content");
    if (!sidebarEl || !contentEl) return;

    // Build Sidebar
    var sb = '<div class="sidebar-group"><div class="sidebar-title">Packages &amp; Modules</div>';
    for (var i = 0; i < data.modules.length; i++) {
      var m = data.modules[i];
      sb += '<a href="#' + m.id + '">' + escapeHtml(m.name) + '</a>';
    }
    sb += '<a href="#manifest-ref" class="nav-text">app.json Manifest &amp; kind</a>';
    sb += '<a href="#env-ref" class="nav-text">Environment Variables</a>';
    sb += '</div>';

    for (var j = 0; j < data.modules.length; j++) {
      var mod = data.modules[j];
      sb += '<div class="sidebar-group"><div class="sidebar-title">' + escapeHtml(mod.name) + '</div>';
      for (var k = 0; k < mod.items.length; k++) {
        var it = mod.items[k];
        sb += '<a href="#' + it.id + '">' + escapeHtml(it.name) + '</a>';
      }
      sb += '</div>';
    }
    sidebarEl.innerHTML = sb;

    // Build Main Content (PyTorch style: Module Summary Tables + <dl class="api-item"> definitions)
    var html = '<h1>' + escapeHtml(data.title) + '</h1>';
    html += '<p class="lead">' + escapeHtml(data.lead) + '</p>';
    html += '<pre><code>' + escapeHtml(data.install) + '</code></pre>';

    for (var mIdx = 0; mIdx < data.modules.length; mIdx++) {
      var moduleObj = data.modules[mIdx];
      html += '<h2 id="' + moduleObj.id + '">' + escapeHtml(moduleObj.name) + '</h2>';
      html += '<p>' + escapeHtml(moduleObj.summary) + '</p>';

      // PyTorch-style Summary Table at top of module
      html += '<table class="summary-table"><thead><tr><th>API</th><th>Description</th></tr></thead><tbody>';
      for (var sIdx = 0; sIdx < moduleObj.items.length; sIdx++) {
        var sItem = moduleObj.items[sIdx];
        html += '<tr><td><a href="#' + sItem.id + '">' + escapeHtml(sItem.name) + '</a></td><td>' + escapeHtml(sItem.desc.split(".")[0] + ".") + '</td></tr>';
      }
      html += '</tbody></table>';

      // PyTorch-style <dl class="api-item"> blocks
      for (var fIdx = 0; fIdx < moduleObj.items.length; fIdx++) {
        var fn = moduleObj.items[fIdx];
        html += '<dl class="api-item" id="' + fn.id + '">';
        html += '<dt class="sig-object"><div><span class="sig-qual">' + escapeHtml(fn.qual) + '</span><span class="sig-name">' + escapeHtml(fn.name) + '</span><span class="sig-param">' + escapeHtml(fn.paramsSig) + '</span>' + (fn.retSig ? ' <span class="sig-ret">-&gt; ' + escapeHtml(fn.retSig) + '</span>' : '') + '</div><a class="headerlink" href="#' + fn.id + '" title="Link to this definition">#</a></dt>';
        html += '<dd class="sig-body">';
        html += '<p>' + escapeHtml(fn.desc) + '</p>';
        html += '<dl class="field-list">';
        if (fn.params && fn.params.length > 0) {
          html += '<dt>Parameters:</dt><dd><ul>';
          for (var pIdx = 0; pIdx < fn.params.length; pIdx++) {
            var p = fn.params[pIdx];
            html += '<li><span class="param-name">' + escapeHtml(p[0]) + '</span> (<span class="param-type">' + escapeHtml(p[1]) + '</span>) &ndash; ' + escapeHtml(p[2]) + '</li>';
          }
          html += '</ul></dd>';
        }
        html += '<dt>Returns:</dt><dd>' + escapeHtml(fn.returns) + '</dd>';
        html += '<dt>Return type:</dt><dd><span class="param-type">' + escapeHtml(fn.rtype) + '</span></dd>';
        if (fn.raises) {
          html += '<dt>Raises / Errors:</dt><dd>' + escapeHtml(fn.raises) + '</dd>';
        }
        html += '</dl>';
        if (fn.example) {
          html += '<div class="example-label">Example:</div>';
          html += '<pre><code>' + escapeHtml(fn.example) + '</code></pre>';
        }
        html += '</dd></dl>';
      }
    }

    // app.json Manifest & kind Reference section
    html += '<h2 id="manifest-ref">app.json Manifest &amp; Parameter <code>kind</code> Reference</h2>';
    html += '<p>Every Yu OS App declares its metadata, parameters, and permissions in <code>app.json</code>. Command-line and desktop launches share <code>operations</code>.</p>';
    html += '<table class="summary-table"><thead><tr><th>Arg <code>kind</code></th><th>Value Type</th><th>Behavior</th></tr></thead><tbody>';
    html += '<tr><td><code>"text"</code></td><td><code>string</code></td><td>Free-form UTF-8 string.</td></tr>';
    html += '<tr><td><code>"path"</code></td><td><code>string</code> (file path)</td><td>Expands <code>./</code> and <code>~/</code> and <strong>automatically grants read access (<code>fs.read</code>)</strong> to the user-supplied file in the sandbox without needing a <code>needs</code> entry.</td></tr>';
    html += '<tr><td><code>"integer"</code></td><td><code>int</code></td><td>Whole number; non-integers are rejected before launch.</td></tr>';
    html += '<tr><td><code>"number"</code></td><td><code>float</code></td><td>Integer or decimal number.</td></tr>';
    html += '<tr><td><code>"bool"</code></td><td><code>boolean</code></td><td>Boolean flag (defaults to <code>--name</code> CLI flag).</td></tr>';
    html += '<tr><td><code>"host"</code></td><td><code>string</code></td><td>Network <code>host</code> or <code>host:port</code>.</td></tr>';
    html += '<tr><td><code>"name"</code></td><td><code>string</code></td><td>Identifier token.</td></tr>';
    html += '</tbody></table>';

    // Environment Variables section
    html += '<h2 id="env-ref">Environment Variables</h2>';
    html += '<table class="summary-table"><thead><tr><th>Variable</th><th>Default</th><th>Description</th></tr></thead><tbody>';
    html += '<tr><td><code>YUOS_DEV_AI_URL</code></td><td>&mdash;</td><td>OpenAI-compatible <code>/v1</code> endpoint (e.g. <code>http://localhost:11434/v1</code>) used outside <code>YUOS_SESSION</code>.</td></tr>';
    html += '<tr><td><code>YUOS_DEV_AI_CHAT_MODEL</code></td><td><code>llama3.2</code></td><td>Model for <code>chat</code> and <code>chat_stream</code>.</td></tr>';
    html += '<tr><td><code>YUOS_DEV_AI_EMBED_MODEL</code></td><td><code>nomic-embed-text</code></td><td>Model for <code>embed</code>.</td></tr>';
    html += '<tr><td><code>YUOS_DEV_AI_TTS_MODEL</code></td><td><code>tts-1</code></td><td>Model for <code>tts</code>.</td></tr>';
    html += '<tr><td><code>YUOS_DEV_AI_STT_MODEL</code></td><td><code>whisper-1</code></td><td>Model for <code>stt</code>.</td></tr>';
    html += '<tr><td><code>YUOS_DEV_AI_IMAGEGEN_MODEL</code></td><td><code>dall-e-3</code></td><td>Model for <code>imagegen</code>.</td></tr>';
    html += '<tr><td><code>YUOS_DEV_AI_DECIDE_MODEL</code></td><td><code>llama3.2</code></td><td>Model for <code>decide</code>.</td></tr>';
    html += '<tr><td><code>YUOS_DEV_AI_MODEL</code></td><td><code>llama3.2</code></td><td>Fallback model when a slot-specific variable above is unset.</td></tr>';
    html += '<tr><td><code>YUOS_DEV_AI_KEY</code></td><td>&mdash;</td><td>Optional Bearer API key for <code>YUOS_DEV_AI_URL</code>.</td></tr>';
    html += '</tbody></table>';

    contentEl.innerHTML = html;
  }

  document.addEventListener("DOMContentLoaded", function () {
    var lang = document.body.getAttribute("data-active-lang") || "python";
    renderApiPage(lang);
    var buttons = document.querySelectorAll(".lang-tab");
    for (var i = 0; i < buttons.length; i++) {
      buttons[i].addEventListener("click", function (e) {
        var selected = e.currentTarget.getAttribute("data-select-lang");
        renderApiPage(selected);
      });
    }
  });
})();
