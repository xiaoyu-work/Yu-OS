"""Mark the functions Yu OS starts for an App's command line and desktop.

An App can have a command-line version, a desktop version, or both::

    from yuos_sdk.app import command, desktop

    @command("summarize")      # yu app <id> summarize "text"
    def summarize(args):
        return {"summary": args[0]}

    @desktop                   # App launcher or yu app <id> --gui
    def open_window(files):
        ...                    # Open your window with any GUI toolkit.

Declare the command under ``operations`` and the window under ``desktop``
in ``app.json``.
"""

from __future__ import annotations

import importlib.util
import json
import os
import sys
from typing import Any, Callable, Dict, List, Optional, TypeVar

__all__ = ["command", "desktop", "run"]

Handler = TypeVar("Handler", bound=Callable[[List[str]], Any])

_commands: Dict[str, Callable[[List[str]], Any]] = {}
_desktop: Optional[Callable[[List[str]], Any]] = None
_dispatched: bool = False


def command(name: str) -> Callable[[Handler], Handler]:
    """Run the decorated function for ``yu app <id> <name>``.

    It receives the command's validated arguments and returns a JSON value.
    """
    if not isinstance(name, str) or not name:
        raise ValueError("command name must be a nonempty string")

    def register(handler: Handler) -> Handler:
        if name in _commands:
            raise ValueError(f"command {name!r} already has a function")
        _commands[name] = handler
        return handler

    return register


def desktop(handler: Handler) -> Handler:
    """Run the decorated function to open the App's desktop window.

    It receives the files the user opened with the App.
    """
    global _desktop
    if _desktop is not None:
        raise ValueError("the desktop window already has a function")
    _desktop = handler
    return handler


def _parse_env_args() -> Optional[List[str]]:
    raw = os.environ.get("YUOS_ARGS_JSON")
    if raw is None or raw == "":
        return None
    try:
        parsed = json.loads(raw)
    except ValueError as error:
        raise RuntimeError(f"YUOS_ARGS_JSON must be an array of strings: {error}") from error
    if not isinstance(parsed, list) or any(not isinstance(item, str) for item in parsed):
        raise RuntimeError("YUOS_ARGS_JSON must be an array of strings")
    return list(parsed)


def run(name: Optional[str] = None, args: Optional[List[str]] = None) -> Any:
    """Call the function for this launch. Yu OS calls this for ``main.py``."""
    global _dispatched
    _dispatched = True
    cli_invoked = name is None
    env_args = _parse_env_args()

    if os.environ.get("YUOS_APP_GUI") == "1":
        if _desktop is None:
            raise RuntimeError("main.py has no @desktop function")
        resolved_args = list(args) if args is not None else (env_args if env_args is not None else [])
        return _desktop(resolved_args)

    if name is None:
        env_cmd = os.environ.get("YUOS_COMMAND")
        if env_cmd:
            name = env_cmd
            resolved_args = list(args) if args is not None else (env_args if env_args is not None else [])
        else:
            cli_args = list(args) if args is not None else list(sys.argv[1:])
            if cli_args and cli_args[0] == "--gui":
                if _desktop is None:
                    raise RuntimeError("main.py has no @desktop function")
                return _desktop(cli_args[1:])
            name = cli_args[0] if cli_args else ""
            resolved_args = env_args if env_args is not None else cli_args[1:]
    else:
        resolved_args = list(args) if args is not None else (env_args if env_args is not None else [])

    handler = _commands.get(name)
    if handler is None:
        raise RuntimeError(f"main.py has no @command({name!r}) function")
    result = handler(resolved_args)
    if cli_invoked and result is not None:
        print(json.dumps(result))
    return result


def _main(argv: Optional[List[str]] = None) -> None:
    cli = list(sys.argv[1:] if argv is None else argv)
    if not cli:
        raise SystemExit("usage: python3 -m yuos_sdk.app <path-to-main.py> [<command>|--gui] [args...]")
    script_path = cli[0]
    sys.modules.setdefault("yuos_sdk.app", sys.modules[__name__])
    app_mod = sys.modules["yuos_sdk.app"]
    app_mod._dispatched = False
    old_argv = sys.argv
    sys.argv = [script_path, *cli[1:]]
    try:
        spec = importlib.util.spec_from_file_location("__main__", script_path)
        if spec is None or spec.loader is None:
            raise RuntimeError(f"cannot load {script_path!r}")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        if not getattr(app_mod, "_dispatched", False):
            entry = getattr(module, "run", None)
            if callable(entry) and entry is not app_mod.run:
                env_args = _parse_env_args()
                if os.environ.get("YUOS_APP_GUI") == "1" or (
                    cli[1:2] == ["--gui"] and not os.environ.get("YUOS_COMMAND")
                ):
                    gui_args = env_args if env_args is not None else cli[2:]
                    entry("--gui", gui_args)
                else:
                    cmd_name = os.environ.get("YUOS_COMMAND") or (cli[1] if len(cli) > 1 else "")
                    cmd_args = env_args if env_args is not None else cli[2:]
                    result = entry(cmd_name, cmd_args)
                    if result is not None:
                        print(json.dumps(result))
            else:
                app_mod.run()
    finally:
        sys.argv = old_argv


if __name__ == "__main__":
    _main()
