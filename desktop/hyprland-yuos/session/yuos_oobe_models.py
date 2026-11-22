"""First-run presentation of the installed Agent setup CLI."""

import json
import os
from pathlib import Path
import re
import shutil
import tempfile
import time
from urllib.parse import urlsplit


MODALITIES = ("text", "stt", "tts", "imagegen", "embed", "decision")
COPILOT = "copilot_github_token.json"


def model_args(entry, catalogue, signed_in):
    """Build the formal CLI command; only the separate stdin value is secret."""
    provider = next((p for p in catalogue.get("providers", []) if p["name"] == entry.get("provider")), None)
    if provider is None:
        raise ValueError("provider")
    model = entry["model"].strip()
    if not model or len(model) > 256 or any(c in model for c in "\x00\r\n"):
        raise ValueError("model")
    args = ["apply", "--provider", provider["name"], "--model", model]
    secret = entry.get("apiKey", "").strip()
    if provider.get("auth_kind") == "oauth_device":
        if not signed_in or secret:
            raise ValueError("signIn")
    elif provider.get("needs_credential"):
        if not secret or len(secret) > 4096 or any(c in secret for c in "\x00\r\n"):
            raise ValueError("apiKey")
        args += ["--api-key-stdin"]
    elif secret:
        raise ValueError("apiKey")

    fields = entry.get("extra", {})
    for spec in provider.get("extra_fields", []):
        key = spec["key"]
        value = fields.get(key, "").strip()
        if not value and not spec["required"]:
            continue
        if not value or spec.get("secret") or any(c.isspace() or c == "\x00" for c in value):
            raise ValueError(key)
        if key == "base_url":
            # Endpoint fields describe a resource, never an embedded credential.
            try:
                url = urlsplit(value)
                _ = url.port
            except ValueError:
                raise ValueError(key) from None
            if (not url.hostname or url.username is not None or any(c in value for c in "?#\\")
                    or url.scheme not in ("https", "http")
                    or (provider["name"] == "azure" and (url.scheme != "https" or url.path not in ("", "/")))):
                raise ValueError(key)
            args += ["--base-url", value]
        elif key == "api_version":
            if not re.fullmatch(r"[a-zA-Z0-9._-]{1,64}", value):
                raise ValueError(key)
            args += ["--api-version", value]
        else:
            raise ValueError(key)
    return args, secret


class Models:
    def __init__(self, gio, glib, send, yu, staging, local):
        self.gio, self.glib, self.send, self.yu = gio, glib, send, yu
        self.staging, self.local = staging, local
        self.child = None
        self.catalogue = {}
        self.committed = None
        self.oauth = None
        self.oauth_timer = 0
        self.signed_in = False

    def run(self, modality, args, callback, stdin="", *, profile=None, credentials=None):
        flags = self.gio.SubprocessFlags
        launcher = self.gio.SubprocessLauncher.new(flags.STDIN_PIPE | flags.STDOUT_PIPE | flags.STDERR_SILENCE)
        if profile is not None:
            launcher.setenv("YUOS_CONFIG_PATH", str(profile / "config/config.json"), True)
            launcher.setenv("YUOS_USER_CONFIG_DIR", str(profile / "config"), True)
            launcher.setenv("YUOS_USER_DATA_DIR", str(profile / "data"), True)
        if credentials is not None:
            launcher.setenv("YUOS_CREDENTIALS_DIR", str(credentials), True)
        try:
            self.child = launcher.spawnv([self.yu, "agent", "setup", modality, *args])
        except self.glib.Error:
            callback({})
            return

        child = self.child

        def timeout():
            child.force_exit()
            return self.glib.SOURCE_CONTINUE

        watchdog = self.glib.timeout_add_seconds(30, timeout)

        def completed(process, result):
            self.glib.source_remove(watchdog)
            self.child = None
            try:
                _, stdout, _ = process.communicate_utf8_finish(result)
                response = json.loads(stdout) if process.get_successful() else {}
            except (self.glib.Error, ValueError):
                response = {}
            callback(response)

        child.communicate_utf8_async(stdin, None, completed)

    def load(self):
        if self.child is not None or self.catalogue:
            return
        self.send(agentState="loading")

        def loaded(response):
            self.catalogue = response.get("modalities", {})
            if not self.catalogue:
                self.send(agentState="unavailable")
            elif self.local:
                self.send(agentState="idle", catalogue=self.catalogue)
            else:
                self.run("all", ["--status"], status)

        def status(response):
            saved = {}
            for name in MODALITIES:
                item = response.get("modalities", {}).get(name, {})
                if item.get("model") and item.get("provider") not in (None, "", "none", "mock") and (
                    item.get("configured") or item.get("ready") or name == "decision"
                ):
                    saved[name] = {key: item[key] for key in ("provider", "model")}
            self.send(agentState="idle", catalogue=self.catalogue, savedModels=saved)

        self.run("all", ["providers"], loaded)

    @property
    def oauth_store(self):
        return self.staging / "oauth/credentials"

    def sign_in(self):
        if self.child is not None or self.oauth is not None or self.signed_in:
            return
        flow = {}
        self.oauth = flow
        self.send(oauthState="starting")

        def started(response):
            if self.oauth is not flow:
                return
            if (not response.get("device_code") or not response.get("user_code")
                    or response.get("verification_uri") != "https://github.com/login/device"
                    or not isinstance(response.get("expires_in"), int)
                    or not isinstance(response.get("interval"), int)):
                self.cancel_sign_in("failed")
                return
            flow.update(code=response["device_code"], expires=time.monotonic() + response["expires_in"],
                        interval=max(5, response["interval"]))
            self.send(userCode=response["user_code"], oauthState="authorizing")
            self.schedule_poll()

        self.run("text", ["oauth-start", "--provider", "copilot"], started,
                 profile=self.staging / "oauth", credentials=self.oauth_store)

    def schedule_poll(self):
        self.oauth_timer = self.glib.timeout_add_seconds(self.oauth["interval"], self.poll_sign_in)

    def poll_sign_in(self):
        self.oauth_timer = 0
        flow = self.oauth
        if time.monotonic() >= flow["expires"]:
            self.cancel_sign_in("failed")
            return self.glib.SOURCE_REMOVE
        if self.child is not None:
            self.schedule_poll()
            return self.glib.SOURCE_REMOVE

        def polled(response):
            if self.oauth is not flow:
                return
            status = response.get("status")
            if status in ("pending", "slow_down"):
                if status == "slow_down":
                    flow["interval"] = max(flow["interval"] + 5, response.get("interval") or 0)
                self.schedule_poll()
            elif status == "ok" and (self.oauth_store / "agent" / COPILOT).is_file():
                self.signed_in = True
                self.cancel_sign_in("loadingModels")
                self.run("text", ["models", "--provider", "copilot"], self.copilot_models,
                         profile=self.staging / "oauth", credentials=self.oauth_store)
            else:
                self.cancel_sign_in("failed")

        self.run("text", ["oauth-poll", "--provider", "copilot", "--device-code-stdin"],
                 polled, flow["code"] + "\n", profile=self.staging / "oauth", credentials=self.oauth_store)
        return self.glib.SOURCE_REMOVE

    def cancel_sign_in(self, state="idle"):
        if self.oauth_timer:
            self.glib.source_remove(self.oauth_timer)
            self.oauth_timer = 0
        if self.oauth is not None and self.child is not None:
            self.child.force_exit()
        self.oauth = None
        self.send(userCode="", oauthState=state)

    def copilot_models(self, response):
        if self.signed_in:
            self.send(copilotModels=[row["name"] for row in response.get("models", [])],
                      signedIn=True, oauthState="idle")

    def discard(self):
        self.cancel_sign_in()
        self.signed_in = False
        record = self.oauth_store / "agent" / COPILOT
        record.unlink(missing_ok=True)
        self.send(signedIn=False, copilotModels=[])

    def apply(self, entries):
        if self.child is not None or self.oauth is not None:
            self.send(agentState="saveFailed", modelError="busy")
            return
        if not isinstance(entries, dict) or set(entries) - set(MODALITIES):
            self.send(agentState="saveFailed", modelError="invalid")
            return

        batch = []
        for modality, entry in entries.items():
            try:
                args, secret = model_args(entry, self.catalogue.get(modality, {}), self.signed_in)
            except ValueError as error:
                self.send(agentState="saveFailed", modelErrorModality=modality,
                          modelError=error.args[0] if error.args else "invalid")
                return
            except (TypeError, KeyError, AttributeError):
                self.send(agentState="saveFailed", modelErrorModality=modality, modelError="invalid")
                return
            batch.append((modality, args, secret))
        if not batch:
            self.send(agentState="saved", appliedModels={})
            return

        trial = None
        saved = {}
        self.send(agentState="saving")
        try:
            if self.local:
                trial = Path(tempfile.mkdtemp(prefix="models-", dir=self.staging))
                if self.committed is not None:
                    shutil.copytree(self.committed, trial, dirs_exist_ok=True)
                if any(args[2] == "copilot" for _, args, _ in batch):
                    target = trial / "data/credentials/agent" / COPILOT
                    target.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
                    shutil.copyfile(self.oauth_store / "agent" / COPILOT, target)
                    target.chmod(0o600)
        except OSError:
            if trial is not None:
                shutil.rmtree(trial)
            self.send(agentState="saveFailed", modelErrorModality=batch[0][0], modelError="write")
            return

        def failed(modality):
            if trial is not None:
                shutil.rmtree(trial)
            self.send(agentState="saveFailed", modelErrorModality=modality, modelError="write",
                      appliedModels={} if self.local else saved)

        def advance():
            if not batch:
                if trial is not None:
                    if self.committed is not None:
                        shutil.rmtree(self.committed)
                    self.committed = trial
                self.send(agentState="saved", appliedModels=saved)
                return
            modality, args, secret = batch.pop(0)
            copilot = args[2] == "copilot"

            def result(response):
                if not response.get("ok"):
                    failed(modality)
                    return
                if copilot and not self.local:
                    try:
                        self.publish_oauth()
                    except OSError:
                        failed(modality)
                        return
                saved[modality] = {key: response[key] for key in ("provider", "model")}
                advance()

            credentials = trial / "data/credentials" if trial else self.oauth_store if copilot else None
            self.run(modality, args, result, secret, profile=trial, credentials=credentials)

        advance()

    def publish_oauth(self):
        # OAuth's CLI already wrote an encrypted record. Move that same owner's token
        # into the real store only after the user chose Continue and `apply` accepted it.
        data = Path(os.environ.get("YUOS_USER_DATA_DIR") or Path.home() / ".local/share/yuos")
        directory = Path(os.environ.get("YUOS_CREDENTIALS_DIR") or data / "credentials") / "agent"
        directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        fd, tmp = tempfile.mkstemp(prefix=".oobe-copilot-", dir=directory)
        try:
            with os.fdopen(fd, "wb") as target, (self.oauth_store / "agent" / COPILOT).open("rb") as source:
                shutil.copyfileobj(source, target)
                target.flush()
                os.fsync(target.fileno())
            os.replace(tmp, directory / COPILOT)
        finally:
            Path(tmp).unlink(missing_ok=True)

    def selection(self):
        if self.committed is None:
            return None
        config = json.loads((self.committed / "config/config.json").read_bytes())
        directory = self.committed / "data/credentials/agent"
        names = {config.get(block, {}).get("api_key_credential") for block in ("agent", *MODALITIES[1:])}
        records = {record.name: json.loads(record.read_bytes()) for record in directory.glob("*.json")
                   if record.stem in names}
        return {"config": config, "credentials": records}
