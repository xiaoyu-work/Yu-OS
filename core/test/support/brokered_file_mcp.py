import base64
import hashlib
import json
import time
from pathlib import Path

from yuos_sdk.files import read_file, replace_file
from yuos_sdk.kernel import KernelDenied, call_json_with_stdin_binary
from yuos_sdk.mcp import App, current_context

app = App.from_manifest()
binary = "/usr/local/bin/yu"


def options():
    ctx = current_context()
    return {
        "binary": binary,
        "deadline_unix_ms": ctx.deadline_unix_ms,
        "check_cancelled": ctx.raise_if_cancelled,
    }


def denied(call):
    try:
        call()
    except KernelDenied:
        return
    raise AssertionError("broker unexpectedly allowed the request")


@app.tool("probe.read")
def read(path, mode):
    assert not Path(path).exists()
    if mode == "reject":
        denied(lambda: read_file(path, **options()))
    else:
        assert not Path(path).parent.exists()
        snapshot = read_file(path, **options())
        assert snapshot.content == b"\xdb" * (32 * 1024 * 1024)
        denied(lambda: read_file(path + ".other", **options()))
        denied(lambda: replace_file(path, snapshot.revision, b"wrong", **options()))
    return {"ok": True}


@app.tool("probe.apply")
def apply(path, mode):
    assert not Path(path).exists() and not Path(path).parent.exists()
    snapshot = read_file(path, **options())
    if mode != "apply":
        assert snapshot.content == b"baseline"
        ctx = current_context()
        data = b"proposed bytes"
        started = call_json_with_stdin_binary(
            binary, ["__app_file", "transfer"], json.dumps({
                "path": path, "action": "write-start", "revision": snapshot.revision,
                "size": len(data), "sha256": "sha256:" + hashlib.sha256(data).hexdigest(),
            }).encode(), deadline_unix_ms=ctx.deadline_unix_ms, check_cancelled=ctx.raise_if_cancelled,
        )
        if mode == "stale":
            call_json_with_stdin_binary(
                binary, ["__app_file", "transfer"], json.dumps({
                    "path": path, "action": "write-chunk", "id": started["id"], "offset": 0,
                    "content_base64": base64.b64encode(data).decode(),
                }).encode(), deadline_unix_ms=ctx.deadline_unix_ms, check_cancelled=ctx.raise_if_cancelled,
            )
            Path("uploaded").touch()
            while not Path("owner-edited").exists():
                ctx.raise_if_cancelled()
                time.sleep(0.02)
        denied(lambda: call_json_with_stdin_binary(
            binary, ["__app_file", "commit"], json.dumps({
                "path": path, "id": started["id"],
            }).encode(), deadline_unix_ms=ctx.deadline_unix_ms, check_cancelled=ctx.raise_if_cancelled,
        ))
    else:
        assert snapshot.content == b"\xdb" * (32 * 1024 * 1024)
        replace_file(path, snapshot.revision, b"\0" + snapshot.content[1:], **options())
        denied(lambda: replace_file(path, snapshot.revision, b"replay", **options()))
    return {"ok": True}


app.serve()
