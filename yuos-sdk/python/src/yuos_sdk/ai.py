"""AI helper for Yu OS Python apps.

``chat`` is the stable model API and shells out to
``yu ai chat --app <id>``. The kernel derives ``ai.chat`` or
``ai.chat.untrusted`` from the prompt origin, then runs capability
checks, budget, safety, and audit before a model sees the prompt.

``yu ai chat`` is deliberately separate from ``yu agent chat``.
``yu agent`` is the kernel's *own* Agent product (REPL, memory,
skills, hooks, sessions, recall). Apps must not use it. ``yu ai``
is the App-developer-facing primitive: raw, gated LLM access with
no loop and no kernel state.

Apps **never** name a verb and do **not** pick the model. The machine owner
configures one
provider/model in ``/etc/yuos/agent.toml`` and every app's call uses
that. ``chat`` exposes ``origin``, ``max_units``, and prompt controls,
never a ``model`` argument.

Typical usage::

    from yuos_sdk import ai

    def handle_summarize(args):
        result = ai.chat(
            prompt=args["body"],
            origin="external-content",   # text came from outside
            max_units=2000,
        )
        return {"summary": result.text, "usage": result.usage}

The helper makes ``app_id`` explicit when the env var
``YUOS_APP_ID`` is missing — that way unit tests can target a specific
app without having to set up a full kernel session.

Why a subprocess and not an in-process check?
---------------------------------------------

Apps are **not** allowed to import provider SDKs (openai, anthropic,
google.generativeai, ...). The kernel enforces this at install time
via ``yu app lint``; the AI helper is the *only* sanctioned route to
a model. Centralising the request in ``yu ai chat`` means the
kernel — not the app — controls budget, safety, and audit.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from dataclasses import dataclass, field
from typing import (
    Any, Callable, Dict, Generic, List, Literal, Mapping, NoReturn, Optional,
    Sequence, TypeVar, cast,
)

from ._transport import WIRE_ARG, WireDenied, WireUnavailable, decode_response
from .generated import (
    AiAppTool,
    AiChatImage,
    AiChatRequest,
    AiInference,
    AiInferenceRequest,
    AiToolTurn,
    DecisionInput,
    EmbedInput,
    ImageInput,
    SpeechInput,
    TranscriptionInput,
    WireDecodeError,
    WireDecimal,
    WireJsonValue,
    encode_wire_json,
    validate_ai,
    validate_ai_chat_request,
    validate_ai_inference,
    validate_ai_inference_request,
    validate_budget_show,
    wire_integer_to_int,
)


# Subprocess timeout — covers every shell-out to the `yu` binary. The
# default is long enough for slow providers but bounded so a hung child
# never blocks the calling app forever.
_DEFAULT_TIMEOUT_S = 60


# ---------------------------------------------------------------------------
# Exceptions
# ---------------------------------------------------------------------------


class AiError(Exception):
    """Base class for every error this module raises."""


class AiUnavailable(AiError):
    """The ``yu`` binary could not be invoked or returned garbage."""


class AiDenied(AiError):
    """A gate (capability / origin / budget) refused the call.

    The ``payload`` attribute holds the structured envelope
    ``yu ai chat`` returned (verb, scope, reason, hint, …) —
    suitable for forwarding back to the agent.
    """

    def __init__(self, payload: Mapping[str, Any]):
        self.payload = dict(payload)
        super().__init__(self.payload.get("error") or "AI call denied")


class AiBudgetExceeded(AiDenied):
    """The per-app monthly budget was exhausted."""


class AiSafetyViolation(AiDenied):
    """The safety pipeline refused the request."""


# ---------------------------------------------------------------------------
# Response shape
# ---------------------------------------------------------------------------


@dataclass
class Usage:
    input_tokens: int = 0
    output_tokens: int = 0
    units: int = 0


@dataclass
class Budget:
    period: str = ""
    units_used: int = 0
    units_cap: int = 0


@dataclass
class BudgetUsage:
    app: str
    period: str
    units_used: int


@dataclass
class Review:
    safety: str = "strict"
    prompt_redacted: bool = False


@dataclass
class ProposedToolCall:
    """A proposed call to an App function; only the App decides whether to execute it."""

    id: str
    name: str
    input: WireJsonValue


Origin = Literal["trusted", "user-input", "external-content"]
_Output = TypeVar("_Output")


@dataclass(frozen=True)
class ModelResponse(Generic[_Output]):
    provider: str
    model: str
    units: int
    output: _Output


@dataclass(frozen=True)
class ChatOutput:
    text: str
    tool_calls: List[ProposedToolCall]
    provider_state: Optional[str]


@dataclass(frozen=True)
class Embedding:
    vector: List[float]


@dataclass(frozen=True)
class Speech:
    format: str
    audio_base64: str
    sample_rate_hz: Optional[int] = None


@dataclass(frozen=True)
class Transcription:
    text: str


@dataclass(frozen=True)
class Image:
    format: str
    image_base64: str


@dataclass(frozen=True)
class Decision:
    answers: Mapping[str, WireJsonValue]


@dataclass
class AiResponse:
    text: str
    model: str
    provider: str
    verb: str = ""
    usage: Usage = field(default_factory=Usage)
    budget: Budget = field(default_factory=Budget)
    review: Review = field(default_factory=Review)
    tool_calls: List[ProposedToolCall] = field(default_factory=list)
    raw: Dict[str, WireJsonValue] = field(default_factory=dict)
    provider_state: Optional[str] = None

    @property
    def units(self) -> int:
        return self.usage.units

    @property
    def output(self) -> ChatOutput:
        return ChatOutput(self.text, self.tool_calls, self.provider_state)


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def chat(
    prompt: str,
    *,
    origin: Origin,
    max_units: Optional[int] = None,
    system: Optional[str] = None,
    app_id: Optional[str] = None,
    tools: Optional[List[str]] = None,
    images: Sequence[AiChatImage] = (),
    functions: Sequence[AiAppTool] = (),
    tool_turns: Sequence[AiToolTurn] = (),
) -> AiResponse:
    """Call the configured chat model with text, optional images and App functions."""
    if not prompt or not prompt.strip():
        raise AiError("chat: prompt must be non-empty")
    if not isinstance(origin, str) or not origin.strip():
        raise AiError("chat: prompt origin is required")
    if images or functions or tool_turns:
        if tools:
            raise AiError("chat: supply App functions in functions")
        if app_id and app_id != os.environ.get("YUOS_APP_ID"):
            raise AiDenied({"code": "PERMISSION_DENIED", "error": "chat: App identity does not match the session"})
        request: AiChatRequest = {"prompt": prompt, "origin": origin}
        if system is not None:
            request["system"] = system
        if max_units is not None:
            request["max_units"] = max_units
        if images:
            request["images"] = list(images)
        if functions:
            request["tools"] = list(functions)
        if tool_turns:
            request["tool_turns"] = list(tool_turns)
        return chat_request(request)
    return _dispatch(
        modality="chat",
        prompt=prompt,
        origin=origin,
        max_units=max_units,
        system=system,
        app_id=app_id,
        tools=tools,
    )


def chat_request(request: AiChatRequest) -> AiResponse:
    """Call the configured text model with optional App functions and prior tool results."""
    data = encode_wire_json(request).encode("utf-8")
    if len(data) > 720 * 1024:
        raise AiError("App chat request exceeds the byte limit")
    validate_ai_chat_request(request)
    return _parse_response(_model_call("chat", "--request-stdin", data=data))


def infer(request: AiInferenceRequest) -> AiInference:
    """Run the owner's configured embedding, speech, image, or decision model."""
    data = encode_wire_json(request).encode("utf-8")
    if len(data) > 720 * 1024:
        raise AiError("AI request exceeds the byte limit")
    validate_ai_inference_request(request)
    reply = _model_call("infer", "--request-stdin", data=data)
    try:
        validate_ai_inference(reply)
    except WireDecodeError as error:
        raise AiUnavailable(str(error)) from error
    if reply["output"]["kind"] != request["input"]["kind"]:
        raise AiUnavailable("AI inference reply does not match the request")
    reply["units"] = wire_integer_to_int(reply["units"])
    return cast(AiInference, reply)


def _run_model(
    input: EmbedInput | SpeechInput | TranscriptionInput | ImageInput | DecisionInput,
    origin: Origin,
    max_units: Optional[int],
    output_type: Callable[..., _Output],
) -> ModelResponse[_Output]:
    request: AiInferenceRequest = {"origin": origin, "input": input}
    if max_units is not None:
        request["max_units"] = max_units
    reply = infer(request)
    output = {key: value for key, value in reply["output"].items() if key != "kind"}
    if "vector" in output:
        output["vector"] = [
            float(value.lexeme if isinstance(value, WireDecimal) else value)
            for value in output["vector"]
        ]
    if "sample_rate_hz" in output:
        output["sample_rate_hz"] = wire_integer_to_int(output["sample_rate_hz"])
    return ModelResponse(reply["provider"], reply["model"], reply["units"], output_type(**output))


def embed(text: str, *, origin: Origin, max_units: Optional[int] = None) -> ModelResponse[Embedding]:
    return _run_model({"kind": "embed", "text": text}, origin, max_units, Embedding)


def tts(text: str, *, origin: Origin, max_units: Optional[int] = None) -> ModelResponse[Speech]:
    return _run_model({"kind": "tts", "text": text}, origin, max_units, Speech)


def stt(
    audio_base64: str, *, format: Literal["wav", "mp3", "ogg"],
    origin: Origin, max_units: Optional[int] = None,
) -> ModelResponse[Transcription]:
    return _run_model(
        {"kind": "stt", "audio_base64": audio_base64, "format": format},
        origin, max_units, Transcription,
    )


def imagegen(prompt: str, *, origin: Origin, max_units: Optional[int] = None) -> ModelResponse[Image]:
    return _run_model({"kind": "imagegen", "prompt": prompt}, origin, max_units, Image)


def decide(
    state: WireJsonValue, questions: Mapping[str, WireJsonValue], *,
    origin: Origin, max_units: Optional[int] = None,
) -> ModelResponse[Decision]:
    return _run_model(
        {"kind": "decision", "state": state, "questions": dict(questions)},
        origin, max_units, Decision,
    )


def open_permission_settings() -> None:
    """Open Applications settings after a user action; this does not authorize AI."""
    _model_call("settings")


def _model_call(command: str, *args: str, data: bytes = b"") -> Dict[str, Any]:
    try:
        proc = subprocess.run(
            [_yu_binary(), WIRE_ARG, "ai", command, *args],
            input=data, capture_output=True, check=False, timeout=_DEFAULT_TIMEOUT_S,
        )
        return decode_response(proc.stdout.decode("utf-8"), proc.returncode, f"yu ai {command}")
    except WireDenied as error:
        _raise_for_error(error.payload)
    except (WireUnavailable, OSError, UnicodeError, subprocess.TimeoutExpired) as error:
        raise AiUnavailable(str(error)) from error


def budget(app_id: Optional[str] = None) -> BudgetUsage:
    """Return usage for the current period; the CLI does not report a cap."""
    app = app_id or os.environ.get("YUOS_APP_ID")
    if not app:
        raise AiError("budget: app_id is required")
    cmd = [_yu_binary(), WIRE_ARG, "agent", "budget", "show", app]
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            check=False,
            timeout=_DEFAULT_TIMEOUT_S,
        )
    except subprocess.TimeoutExpired as exc:
        raise AiUnavailable(
            f"yu agent budget show timed out after {_DEFAULT_TIMEOUT_S}s"
        ) from exc
    text = (proc.stdout or "").strip()
    try:
        env = decode_response(text, proc.returncode, "yu agent budget show")
    except (WireDenied, WireUnavailable) as exc:
        raise AiUnavailable(str(exc)) from exc
    try:
        validate_budget_show(env)
    except WireDecodeError as exc:
        raise AiUnavailable(f"budget response decode failed: {exc}") from exc
    return BudgetUsage(
        app=env["app"],
        period=env["period"],
        units_used=wire_integer_to_int(env["units_used"]),
    )


# ---------------------------------------------------------------------------
# Internals
# ---------------------------------------------------------------------------


def _dispatch(
    *,
    modality: str,
    prompt: Optional[str],
    origin: str,
    max_units: Optional[int],
    app_id: Optional[str],
    system: Optional[str] = None,
    tools: Optional[List[str]] = None,
) -> AiResponse:
    """Build the stable ``yu ai chat`` command and parse its envelope."""
    app = app_id or os.environ.get("YUOS_APP_ID")
    if not app:
        raise AiError(
            f"{modality}: app_id is required (pass app_id= or set YUOS_APP_ID)"
        )

    with tempfile.TemporaryDirectory(prefix="yuos-ai-") as private_dir:
        cmd = [
            _yu_binary(),
            WIRE_ARG,
            "ai",
            "chat",
            "--app",
            app,
            "--origin",
            origin,
        ]

        def add_private_file(flag: str, name: str, value: str) -> None:
            path = os.path.join(private_dir, name)
            fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, "w", encoding="utf-8") as output:
                output.write(value)
                output.flush()
                os.fsync(output.fileno())
            cmd.extend([flag, path])

        if prompt is not None:
            add_private_file("--prompt-file", "prompt", prompt)
        if max_units is not None:
            cmd.extend(["--max-units", str(max_units)])
        if system is not None:
            add_private_file("--system-file", "system", system)
        if tools:
            cmd.extend(["--tools", ",".join(tools)])

        try:
            proc = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                check=False,
                timeout=_DEFAULT_TIMEOUT_S,
            )
        except subprocess.TimeoutExpired as exc:
            raise AiUnavailable(
                f"yu ai {modality} timed out after {_DEFAULT_TIMEOUT_S}s"
            ) from exc
    payload_text = (proc.stdout or "").strip()
    try:
        envelope = decode_response(payload_text, proc.returncode, "yu ai chat")
    except WireDenied as exc:
        _raise_for_error(exc.payload)
    except WireUnavailable as exc:
        raise AiUnavailable(str(exc)) from exc

    return _parse_response(envelope)


def _parse_response(env: Any) -> AiResponse:
    try:
        validate_ai(env)
    except WireDecodeError as exc:
        raise AiUnavailable(f"ai response decode failed: {exc}") from exc
    usage = env["usage"]
    budget_blk = env["budget"]
    review = env["review"]
    raw_calls = env.get("tool_calls", [])
    parsed_calls: List[ProposedToolCall] = []
    for tc in raw_calls:
        parsed_calls.append(
            ProposedToolCall(
                id=tc["id"],
                name=tc["name"],
                input=tc["input"],
            )
        )
    return AiResponse(
        text=env["text"],
        model=env["model"],
        provider=env["provider"],
        verb=env["verb"],
        usage=Usage(
            input_tokens=wire_integer_to_int(usage["input_tokens"]),
            output_tokens=wire_integer_to_int(usage["output_tokens"]),
            units=wire_integer_to_int(usage["units"]),
        ),
        budget=Budget(
            period=budget_blk["period"],
            units_used=wire_integer_to_int(budget_blk["units_used"]),
            units_cap=wire_integer_to_int(budget_blk["units_cap"]),
        ),
        review=Review(
            safety=review["safety"],
            prompt_redacted=review["prompt_redacted"],
        ),
        tool_calls=parsed_calls,
        raw=dict(env),
        provider_state=env.get("provider_state"),
    )


def _raise_for_error(env: Mapping[str, Any]) -> NoReturn:
    code = str(env["code"])
    if code == "BUDGET_EXCEEDED":
        raise AiBudgetExceeded(env)
    if code == "SAFETY_VIOLATION":
        raise AiSafetyViolation(env)
    raise AiDenied(env)


def _yu_binary() -> str:
    override = os.environ.get("YUOS_KERNEL_BIN")
    if override:
        return override
    found = shutil.which("yu")
    if found is None:
        raise AiUnavailable(
            "the `yu` binary is not on PATH; cannot reach the AI gate"
        )
    return found
