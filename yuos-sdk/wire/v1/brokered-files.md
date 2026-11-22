# Brokered existing files

An App MCP tool can ask Root to read one existing owner file without mounting
the file or its parent in the tool's sandbox. Mark a need on a scalar `path`
argument with no default:

```json
{
  "verb": "fs.read",
  "scope": {"kind": "from-arg", "arg": "path", "transform": "brokered-file"},
  "why": {"en": "Read the document you selected"}
}
```

For a tool that replaces the file, add a separate `fs.write` need with the
same binding and a write purpose. These are ordinary exact capabilities for
review and policy, not grants. Root requires either an already delegated exact
write or the existing protected owner approval for that exact write; a broad
parent write delegation, MCP argument, or App confirmation flag is not that
approval. Approval grants the scoped operation, not approval of the replacement
bytes. Ordinary operation and GUI manifests cannot use this placement.

The argument must be supplied as an absolute literal path. Root does not
expand or canonicalize it into a different name. The authorized invocation
runs in a single-call sandbox; it cannot mix brokered access with raw path
needs. The marked capabilities do not reach mounts, the session's ordinary
capability set, or legacy file providers.

## Python SDK

```python
from yuos_sdk.files import read_file, replace_file
from yuos_sdk.mcp import current_context

ctx = current_context()
options = {
    "binary": "/usr/local/bin/yu",
    "deadline_unix_ms": ctx.deadline_unix_ms,
    "check_cancelled": ctx.raise_if_cancelled,
}
snapshot = read_file(path, **options)
# Work with snapshot.content in App-private scratch. Never open path directly.
replace_file(path, snapshot.revision, edited_bytes, **options)
```

The revision names the complete read in this invocation. It is opaque and
cannot be saved for another call, used for a different file, or replayed after
commit. Read-only tools omit the write need and never call `replace_file`.
Both the snapshot and replacement are binary, up to 32 MiB each (including
empty files). No directory, create, delete, symlink, multi-link or special-file
operation is supported. A replacement must be able to preserve ownership,
group and ordinary mode; special modes and files with extended attributes are
not replaceable. Root opens the ordinary file and its parents under the
authenticated owner's filesystem credentials. Working copies live on the App
side, not in the host directory tree.

The controlled CLI transports at most 512 KiB of binary data per frame. Root
keeps private, ordered transfer state within the live grant and retires it
with the invocation. Every step spends the current exact capability; only a
complete, verified upload can reach commit. Before a replacement, Root
compares the path's pinned parent, file identity, metadata and content digest
with the snapshot. It exchanges two existing entries atomically, then syncs
the directory. Filesystems without a supported exchange are refused.

This is not a filesystem compare-and-swap against a writer that ignores the
broker. A host writer can race the last check; if Root observes an unexpected
displaced entry after the exchange it preserves that entry and reports an
indeterminate outcome. Lost commit replies can also be indeterminate. Never
automatically retry. A revision is not consent, an OS undo token, or proof of
what a later read will see. This contract does not cover active in-process
documents, and source availability does not publish an SDK or platform.
