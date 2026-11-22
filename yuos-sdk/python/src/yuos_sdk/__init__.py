"""yuos_sdk — public AI/agent SDK for Yu OS, Python edition.

This package is the AI-facing surface a third-party Linux developer
uses when their app touches the kernel's AI features. Everything in
here mirrors a member of the wire protocol at ``yuos-sdk/wire/v1/``
and is generated or hand-written to satisfy that contract.

Public modules
--------------
- :mod:`yuos_sdk.ai`       — text chat through ``yu ai chat``.
- :mod:`yuos_sdk.tools`    — call fixed kernel catalog tools
  (``tools.call``, ``tools.catalog``, ``tools.for_chat``), not other Apps.
- :mod:`yuos_sdk.gui`      — desktop GUI bootstrap: the kernel context
  handed to an app launched via ``yu app <id> --gui`` (toolkit-agnostic).
- :mod:`yuos_sdk.mcp`      — manifest-bound MCP App service runtime.
- :mod:`yuos_sdk.kernel`   — cancellable installed-CLI primitive transport.
- :mod:`yuos_sdk.objects`  — pure App object identifiers, without discovery,
  filesystem access, or dispatch.
- :mod:`yuos_sdk.yuos_session` — reference implementation for
  third-party agents that want to attach to a live ``yuos`` session.
- :mod:`yuos_sdk.generated` — typed wire bindings emitted from the
  wire-v1 JSON Schemas. Do not hand-edit; re-run
  ``python3 yuos-sdk/wire/codegen.py`` instead.

Typical usage::

    from yuos_sdk import ai, tools

    def cmd_summarise(args):
        text = open(args["path"]).read()
        return ai.chat(f"Summarise: {text}", origin="external-content")

Capability gating (``policy.require``) and the COW snapshot helper
live in the **internal** ``yuos_runtime`` package — they are
implementation details of the yuos bundled apps, not part of the
public SDK surface.
"""

from . import ai, channels, events, gui, kernel, manifest, mcp, objects, tools

__all__ = ["ai", "channels", "events", "gui", "kernel", "manifest", "mcp", "objects", "tools"]
__version__ = "0.1.0"
