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

import os
from typing import Any, Callable, Dict, List, Optional, TypeVar

__all__ = ["command", "desktop"]

Handler = TypeVar("Handler", bound=Callable[[List[str]], Any])

_commands: Dict[str, Callable[[List[str]], Any]] = {}
_desktop: Optional[Callable[[List[str]], Any]] = None


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


def run(name: str, args: List[str]) -> Any:
    """Call the function for this launch. Yu OS calls this for ``main.py``."""
    if os.environ.get("YUOS_APP_GUI") == "1":
        if _desktop is None:
            raise RuntimeError("main.py has no @desktop function")
        return _desktop(list(args))
    handler = _commands.get(name)
    if handler is None:
        raise RuntimeError(f"main.py has no @command({name!r}) function")
    return handler(list(args))
