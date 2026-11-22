"""Present the Root worker's first-run update stages."""

import json
import os
from pathlib import Path
import sys


ROOT = Path("/var/lib/yuos-updates")
STATUS = ROOT / "setup.json"
COMPLETED = ROOT / "setup-complete"
RESUME = ROOT / f"resume-{os.getuid()}"
DRIVER_RUNNING = Path("/run/yuos-driver-online")
REQUEST = ("/usr/bin/pkexec", "--disable-internal-agent", "/usr/libexec/yuos-setup-update")
DRIVERS = {"checking", "installing", "ready", "offline", "failed", "required-offline", "required-failed"}
SOFTWARE = {"waiting", "checking", "downloading", "restart", "restarting", "current", "installed",
            "offline", "failed", "restart-failed", "repair"}
FINISH = {"current", "installed", "offline", "failed", "restart-failed"}
RUNNING = {"checking", "downloading"}


class Updates:
    def __init__(self, Gio, GLib, send):
        self.Gio, self.GLib, self.send = Gio, GLib, send
        self.selected = False
        self.online = False
        self.view = None
        self.snapshot = None
        self.child = None
        self.reconnected = False

    def read(self):
        try:
            value = json.loads(STATUS.read_text())
            drivers, software = value["drivers"], value["software"]
            if drivers not in DRIVERS or software not in SOFTWARE:
                raise ValueError("unknown phase")
            return drivers, software
        except FileNotFoundError:
            return None
        except (OSError, ValueError, KeyError, TypeError) as error:
            print(f"Yu OS: unreadable setup update status: {error}", file=sys.stderr)
            return "failed", "repair"

    def poll(self):
        self.view = self.read()
        view = ("installing", "waiting") if DRIVER_RUNNING.is_dir() else self.view
        if self.snapshot != view:
            self.snapshot = view
            drivers, software = view or ("checking", "waiting")
            self.send(driverPhase=drivers, softwarePhase=software, updateError="")
        return self.GLib.SOURCE_CONTINUE

    @property
    def software(self):
        return self.view[1] if self.view else "waiting"

    def enter(self):
        if self.selected:
            return
        self.selected = True
        self.poll()
        self.GLib.timeout_add(500, self.poll)
        if (self.view is None or self.software in RUNNING | {"restarting"}
                or self.online and self.can_retry_network()):
            self.start()

    def can_retry_network(self):
        return self.view is not None and (self.view[0] in {"offline", "required-offline"}
                                         or self.software in {"offline", "failed"})

    def connectivity(self, online):
        reconnected = online and not self.online
        self.online = online
        if not reconnected or not self.selected:
            return
        if self.child is not None:
            self.reconnected = True
        elif self.can_retry_network():
            self.start()

    def start(self):
        if self.child is not None:
            return
        before = self.view
        self.send(updateError="", updateRequest=True)

        def started(success):
            self.poll()
            projected = self.view is not None and self.view != before
            self.send(updateRequest=False, updateError="" if success or projected else "unavailable")
            reconnected, self.reconnected = self.reconnected, False
            if reconnected and self.online and success and self.can_retry_network():
                self.start()

        self.request("start", started)

    def allows(self, restart):
        if not self.selected or self.child is not None or DRIVER_RUNNING.is_dir():
            return False
        self.poll()
        return (self.view is not None and self.view[0] in {"ready", "offline", "failed"}
                and (self.software == "restart" if restart else self.software in FINISH))

    def request(self, action, settled):
        if self.child is not None:
            settled(False)
            return
        try:
            flags = self.Gio.SubprocessFlags
            launcher = self.Gio.SubprocessLauncher.new(flags.NONE)
            launcher.unsetenv("SHELL")
            self.child = launcher.spawnv([*REQUEST, action])
        except self.GLib.Error:
            settled(False)
            return

        def received(child, result):
            self.child = None
            try:
                success = child.wait_check_finish(result)
            except self.GLib.Error:
                success = False
            settled(success)

        self.child.wait_check_async(None, received)
