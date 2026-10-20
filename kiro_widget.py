#!/usr/bin/env python3
"""Kiro Crew widget: a tray icon with controls for the kirocrew gateway.

Everything goes through the `kirocrew` CLI, which is service-aware (systemd unit
or foreground gateway), so the widget never talks to systemd directly. Status is
a cheap TCP probe of the dashboard port plus a /proc scan, not a CLI call, so
polling doesn't spawn a Python interpreter every few seconds.
"""
from __future__ import annotations

import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

from PyQt6.QtCore import QProcess, QTimer, Qt, QUrl
from PyQt6.QtGui import QAction, QColor, QCursor, QDesktopServices, QFont, QIcon, QPainter, QPixmap
from PyQt6.QtWidgets import (
    QApplication, QDialog, QMenu, QMessageBox, QPlainTextEdit, QSystemTrayIcon, QVBoxLayout,
)

PORT = int(os.environ.get("KIROCREW_PORT", "5476"))
POLL_MS = 3000
KIROCREW = shutil.which("kirocrew") or str(Path.home() / ".local/bin/kirocrew")
CLK_TCK = os.sysconf("SC_CLK_TCK")

GREEN, RED, AMBER = "#3fb950", "#f85149", "#d29922"


def port_open(port: int) -> bool:
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=0.4):
            return True
    except OSError:
        return False


def gateway_pid() -> int | None:
    """PID of the `kirocrew gateway` process, found by scanning /proc."""
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            argv = (entry / "cmdline").read_bytes().split(b"\0")
        except OSError:
            continue
        if any(a.endswith(b"kirocrew") for a in argv[:3]) and b"gateway" in argv:
            return int(entry.name)
    return None


def uptime_of(pid: int) -> str:
    try:
        start_ticks = int(Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[19])
        boot = float(Path("/proc/uptime").read_text().split()[0])
        secs = int(boot - start_ticks / CLK_TCK)
    except (OSError, ValueError, IndexError):
        return "?"
    d, rem = divmod(secs, 86400)
    h, rem = divmod(rem, 3600)
    m = rem // 60
    return " ".join(p for p in (f"{d}d" if d else "", f"{h}h" if h else "", f"{m}m") if p)


def dot_icon(color: str) -> QIcon:
    pm = QPixmap(64, 64)
    pm.fill(Qt.GlobalColor.transparent)
    p = QPainter(pm)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    p.setBrush(QColor(color))
    p.setPen(Qt.PenStyle.NoPen)
    p.drawEllipse(8, 8, 48, 48)
    p.end()
    return QIcon(pm)


class OutputDialog(QDialog):
    """Popup showing a CLI command's output; deletes itself on close."""

    def __init__(self, title: str) -> None:
        super().__init__()
        self.setWindowTitle(title)
        self.setAttribute(Qt.WidgetAttribute.WA_DeleteOnClose)
        self.resize(640, 420)
        self.text = QPlainTextEdit(readOnly=True)
        self.text.setFont(QFont("monospace", 9))
        QVBoxLayout(self).addWidget(self.text)


class TrayWidget:
    def __init__(self) -> None:
        self.running = False
        self.busy = False
        self.tray = QSystemTrayIcon(dot_icon(AMBER))
        self.menu = QMenu()

        self.a_status = QAction("Checking…")
        self.a_status.setEnabled(False)
        self.menu.addAction(self.a_status)
        self.menu.addSeparator()
        self.a_dash = self.menu.addAction("Beam me up 🚀", self.open_dashboard)
        self.a_start = self.menu.addAction("Start", self.start)
        self.a_stop = self.menu.addAction("Stop", self.stop)
        self.menu.addSeparator()
        self.a_logs = self.menu.addAction("Logs", lambda: self.show_cli("Logs", "logs", "-n", "100"))
        self.a_doctor = self.menu.addAction("Doctor", lambda: self.show_cli("Doctor", "doctor"))
        self.a_cron = self.menu.addAction("Cron jobs", lambda: self.show_cli("Cron jobs", "cron", "list"))
        self.a_spawn = self.menu.addAction("Subagents", lambda: self.show_cli("Subagents", "spawn", "list"))
        self.menu.addSeparator()
        self.menu.addAction("Quit widget", QApplication.instance().quit)

        self.tray.setContextMenu(self.menu)
        # Left click opens the same menu as right click.
        self.tray.activated.connect(
            lambda r: self.menu.popup(QCursor.pos()) if r == QSystemTrayIcon.ActivationReason.Trigger else None)
        self.tray.show()

        self.timer = QTimer(interval=POLL_MS)
        self.timer.timeout.connect(self.refresh)
        self.timer.start()
        self.refresh()

    # -- status -------------------------------------------------------------
    def refresh(self) -> None:
        self.running = port_open(PORT)
        pid = gateway_pid() if self.running else None
        if self.busy:
            color, text = AMBER, "Working…"
        elif self.running:
            color = GREEN
            text = "Running" + (f" · pid {pid} · up {uptime_of(pid)}" if pid else f" · port {PORT}")
        else:
            color, text = RED, "Stopped"
        self.tray.setIcon(dot_icon(color))
        self.tray.setToolTip(f"Kiro Crew: {text}")
        self.a_status.setText(f"Kiro Crew: {text}")
        idle = not self.busy
        self.a_start.setEnabled(idle and not self.running)
        self.a_stop.setEnabled(idle and self.running)
        self.a_dash.setEnabled(self.running)
        self.a_logs.setEnabled(True)
        self.a_doctor.setEnabled(True)
        self.a_cron.setEnabled(self.running)
        self.a_spawn.setEnabled(self.running)

    def notify(self, msg: str) -> None:
        self.tray.showMessage("Kiro Crew", msg, QSystemTrayIcon.MessageIcon.Information, 4000)

    # -- actions ------------------------------------------------------------
    def start(self) -> None:
        if self.running or self.busy:
            return
        # Prefer the systemd unit when installed; otherwise run a detached foreground gateway.
        unit = subprocess.run(["systemctl", "cat", "kirocrew.service"], capture_output=True).returncode == 0
        if unit and shutil.which("pkexec"):
            cmd = ["pkexec", "systemctl", "start", "kirocrew.service"]
        else:
            cmd = [KIROCREW, "gateway", "--no-open"]
        subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL, start_new_session=True)
        self._wait_for(True, "Gateway is up.", "Gateway did not come up within 30s. Check Logs / Doctor.")

    def _wait_for(self, up: bool, ok_msg: str, fail_msg: str, timeout: int = 30) -> None:
        deadline = time.monotonic() + timeout
        self.busy = True
        t = QTimer(interval=500)

        def tick():
            if port_open(PORT) == up:
                t.stop(); t.deleteLater(); self.busy = False; self.notify(ok_msg); self.refresh()
            elif time.monotonic() > deadline:
                t.stop(); t.deleteLater(); self.busy = False; self.notify(fail_msg); self.refresh()
        t.timeout.connect(tick)
        t.start()
        self._wait_timer = t  # keep a reference
        self.refresh()

    def stop(self) -> None:
        if not self.running or self.busy:
            return
        r = QMessageBox.question(None, "Kiro Crew", "Stop the gateway?\n\nThis interrupts running Kiro sessions.")
        if r != QMessageBox.StandardButton.Yes:
            return
        p = QProcess()
        p.setProgram(KIROCREW)
        p.setArguments(["stop"])
        p.finished.connect(lambda *_: self._wait_for(False, "Gateway stopped.", "Gateway still answering after stop."))
        self._stop_proc = p
        self.busy = True
        p.start()
        self.refresh()

    def open_dashboard(self) -> None:
        # `kirocrew token` prints an authenticated URL; it is opened, never shown or logged.
        r = subprocess.run([KIROCREW, "token"], capture_output=True, text=True)
        url = next((l.strip() for l in r.stdout.splitlines() if l.strip().startswith("http")), f"http://localhost:{PORT}")
        QDesktopServices.openUrl(QUrl(url))

    def show_cli(self, title: str, *args: str) -> None:
        dlg = OutputDialog(f"Kiro Crew · {title}")
        dlg.text.setPlainText(f"$ kirocrew {' '.join(args)}\n")
        p = QProcess(dlg)
        p.setProgram(KIROCREW)
        p.setArguments(list(args))
        p.setProcessChannelMode(QProcess.ProcessChannelMode.MergedChannels)

        def drain():
            # The CLI prints config-deprecation WARNING noise on every call; drop it.
            for line in bytes(p.readAllStandardOutput()).decode(errors="replace").splitlines():
                if " WARNING kiro_crew.config.loader" not in line:
                    dlg.text.appendPlainText(line)
        p.readyReadStandardOutput.connect(drain)
        p.finished.connect(lambda code, _s: (drain(), code and dlg.text.appendPlainText(f"[exit {code}]")))
        p.start()
        dlg.show()


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationName("kiro-widget")
    app.setQuitOnLastWindowClosed(False)
    if not QSystemTrayIcon.isSystemTrayAvailable():
        print("No system tray available.", file=sys.stderr)
        return 1
    _widget = TrayWidget()  # noqa: F841 (kept alive for the app's lifetime)
    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
