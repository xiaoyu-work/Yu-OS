"""Mark the functions Yu OS starts for an App's command line and desktop.

Declare operations once under ``operations`` in ``app.json``; command-line and
desktop launches share that declaration. If an App only implements
``@command`` or only implements ``@desktop``, the SDK returns a default
unavailable-surface response when the other surface is launched::

    from yuos_sdk.app import command, desktop

    @command                   # yu app <id> summarize "text"
    def summarize(args):
        return {"summary": args[0]}

    @desktop                   # App launcher or yu app <id> --gui
    def main():
        ...                    # Open your window with any GUI toolkit.
"""

from __future__ import annotations

import importlib.util
import inspect
import json
import os
import sys
import time
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, TypeVar, Union, overload

from .kernel import call_json_with_stdin_binary

__all__ = ["command", "data_dir", "desktop", "notify", "run"]

Handler = TypeVar("Handler", bound=Callable[..., Any])

_commands: Dict[str, Callable[[List[str]], Any]] = {}
_desktop_commands: Dict[str, Callable[..., Any]] = {}
_desktop: Optional[Callable[..., Any]] = None
_dispatched: bool = False


@overload
def command(name: Handler) -> Handler: ...


@overload
def command(name: str) -> Callable[[Handler], Handler]: ...


def command(name: Union[Handler, str]) -> Any:
    """Run the decorated function for ``yu app <id> <name>``.

    Can be used as ``@command`` (uses the function name) or
    ``@command("name")``. It receives the command's validated arguments and
    returns a JSON value.
    """
    if callable(name):
        fn = name
        cmd_name = getattr(fn, "__name__", "")
        if not cmd_name:
            raise ValueError("command function must have a name")
        if cmd_name in _commands:
            raise ValueError(f"command {cmd_name!r} already has a function")
        _commands[cmd_name] = fn
        return fn

    if not isinstance(name, str) or not name:
        raise ValueError("command name must be a nonempty string")

    def register(handler: Handler) -> Handler:
        if name in _commands:
            raise ValueError(f"command {name!r} already has a function")
        _commands[name] = handler
        return handler

    return register


@overload
def desktop(handler: Handler) -> Handler: ...


@overload
def desktop(handler: str) -> Callable[[Handler], Handler]: ...


def desktop(handler: Union[Handler, str]) -> Any:
    """Run the decorated function to open the App's desktop window."""
    global _desktop
    if isinstance(handler, str):
        name = handler
        if not name:
            raise ValueError("desktop operation name must be a nonempty string")

        def register(fn: Handler) -> Handler:
            if name in _desktop_commands:
                raise ValueError(f"desktop operation {name!r} already has a function")
            _desktop_commands[name] = fn
            return fn

        return register

    if _desktop is not None:
        raise ValueError("the desktop window already has a function")
    _desktop = handler
    return handler


def data_dir() -> Path:
    """Return the App's private data directory, creating it if needed."""
    env_dir = os.environ.get("YUOS_DATA_DIR")
    if env_dir:
        path = Path(env_dir)
    else:
        xdg = os.environ.get("XDG_DATA_HOME")
        base = Path(xdg) if xdg else (Path.home() / ".local" / "share")
        app_id = os.environ.get("YUOS_APP_ID") or "dev"
        path = base / "yuos-dev" / "apps" / app_id
    path.mkdir(parents=True, exist_ok=True)
    return path


def notify(summary: str, body: str = "") -> Dict[str, Any]:
    """Post a system notification from this App (`ui.notify`)."""
    if not isinstance(summary, str) or not summary.strip():
        raise ValueError("notify: summary must be a non-empty string")
    if not os.environ.get("YUOS_SESSION"):
        return {"status": "dev", "summary": summary, "body": body}
    payload = {
        "action": "post",
        "summary": summary,
        "body": body,
        "app_name": os.environ.get("YUOS_APP_ID") or "",
        "icon": "",
        "expire_ms": 0,
        "transient": False,
    }
    deadline = time.time_ns() // 1_000_000 + 10_000
    return call_json_with_stdin_binary(
        os.environ.get("YUOS_KERNEL_BIN", "/usr/local/bin/yu"),
        ["__notifications", "request", "--request-stdin", "--deadline", str(deadline)],
        json.dumps(payload).encode("utf-8"),
        deadline_unix_ms=deadline,
    )


def _call_desktop_fn(fn: Callable[..., Any], resolved_args: List[str]) -> Any:
    try:
        sig = inspect.signature(fn)
        params = [
            p
            for p in sig.parameters.values()
            if p.kind in (inspect.Parameter.POSITIONAL_ONLY, inspect.Parameter.POSITIONAL_OR_KEYWORD)
            or p.kind == inspect.Parameter.VAR_POSITIONAL
        ]
        if not params:
            return fn()
    except (TypeError, ValueError):
        pass
    return fn(resolved_args)


def _default_unavailable(surface: str, command_name: Optional[str] = None) -> Dict[str, Any]:
    if surface == "desktop":
        payload: Dict[str, Any] = {
            "available": False,
            "surface": "desktop",
            "message": "This feature is only available on the command line.",
        }
    else:
        payload = {
            "available": False,
            "surface": "cli",
            "message": "This feature is only available in the desktop app.",
        }
    if command_name:
        payload["command"] = command_name
    return payload


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


def _run_desktop(resolved_args: List[str], cli_invoked: bool) -> Any:
    if resolved_args and resolved_args[0] in _desktop_commands:
        result = _call_desktop_fn(_desktop_commands[resolved_args[0]], resolved_args[1:])
    elif _desktop is not None:
        result = _call_desktop_fn(_desktop, resolved_args)
    elif len(_desktop_commands) == 1:
        only_fn = next(iter(_desktop_commands.values()))
        result = _call_desktop_fn(only_fn, resolved_args)
    elif _commands:
        result = _default_unavailable("desktop")
    else:
        raise RuntimeError("main.py has no @desktop function")
    if cli_invoked and result is not None:
        print(json.dumps(result))
    return result


def run(name: Optional[str] = None, args: Optional[List[str]] = None) -> Any:
    """Call the function for this launch. Yu OS calls this for ``main.py``."""
    global _dispatched
    _dispatched = True
    cli_invoked = name is None
    env_args = _parse_env_args()

    if os.environ.get("YUOS_APP_GUI") == "1":
        resolved_args = list(args) if args is not None else (env_args if env_args is not None else [])
        return _run_desktop(resolved_args, cli_invoked)

    if name is None:
        env_cmd = os.environ.get("YUOS_COMMAND")
        if env_cmd:
            name = env_cmd
            resolved_args = list(args) if args is not None else (env_args if env_args is not None else [])
        else:
            cli_args = list(args) if args is not None else list(sys.argv[1:])
            if cli_args and cli_args[0] == "--gui":
                return _run_desktop(cli_args[1:], cli_invoked)
            name = cli_args[0] if cli_args else ""
            resolved_args = env_args if env_args is not None else cli_args[1:]
    else:
        resolved_args = list(args) if args is not None else (env_args if env_args is not None else [])

    handler = _commands.get(name)
    if handler is None:
        if _desktop is not None or _desktop_commands:
            result = _default_unavailable("cli", name or None)
            if cli_invoked:
                print(json.dumps(result))
            return result
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
