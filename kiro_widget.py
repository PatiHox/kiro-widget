#!/usr/bin/env python3
"""Kiro Crew widget: a small tray + window control panel for the kirocrew gateway.

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

from PyQt6.QtCore import QProcess, QProcessEnvironment, QTimer, Qt, QUrl
from PyQt6.QtGui import QAction, QColor, QDesktopServices, QFont, QIcon, QPainter, QPixmap
from PyQt6.QtWidgets import (
    QApplication, QHBoxLayout, QLabel, QMainWindow, QMenu, QMessageBox,
    QPlainTextEdit, QPushButton, QSystemTrayIcon, QVBoxLayout, QWidget,
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


class Widget(QMainWindow):
    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("Kiro Crew")
        self.resize(520, 560)
        self.running = False
        self.busy: QProcess | None = None
        self.follower: QProcess | None = None

        root = QWidget()
        self.setCentralWidget(root)
        col = QVBoxLayout(root)

        head = QHBoxLayout()
        self.dot = QLabel("●")
        self.dot.setFont(QFont("", 22))
        self.state = QLabel("Checking…")
        self.state.setFont(QFont("", 14, QFont.Weight.DemiBold))
        self.detail = QLabel("")
        self.detail.setStyleSheet("color: gray")
        titles = QVBoxLayout()
        titles.setSpacing(0)
        titles.addWidget(self.state)
        titles.addWidget(self.detail)
        head.addWidget(self.dot)
        head.addLayout(titles, 1)
        col.addLayout(head)

        row1 = QHBoxLayout()
        self.b_start = self._btn("Start", self.start)
        self.b_stop = self._btn("Stop", self.stop)
        self.b_restart = self._btn("Restart", self.restart)
        self.b_dash = self._btn("Open dashboard", self.open_dashboard)
        for b in (self.b_start, self.b_stop, self.b_restart, self.b_dash):
            row1.addWidget(b)
        col.addLayout(row1)

        row2 = QHBoxLayout()
        self.b_logs = self._btn("Logs", lambda: self.run_cli("logs", "-n", "100"))
        self.b_follow = self._btn("Follow logs", self.toggle_follow)
        self.b_doctor = self._btn("Doctor", lambda: self.run_cli("doctor"))
        self.b_cron = self._btn("Cron jobs", lambda: self.run_cli("cron", "list"))
        self.b_spawn = self._btn("Subagents", lambda: self.run_cli("spawn", "list"))
        for b in (self.b_logs, self.b_follow, self.b_doctor, self.b_cron, self.b_spawn):
            row2.addWidget(b)
        col.addLayout(row2)

        self.out = QPlainTextEdit()
        self.out.setReadOnly(True)
        self.out.setFont(QFont("monospace", 9))
        self.out.setMaximumBlockCount(5000)
        col.addWidget(self.out, 1)

        self._setup_tray()
        self.timer = QTimer(self, interval=POLL_MS, timeout=self.refresh)
        self.timer.start()
        self.refresh()

    # -- ui helpers ---------------------------------------------------------
    def _btn(self, text: str, slot) -> QPushButton:
        b = QPushButton(text)
        b.clicked.connect(lambda _=False: slot())
        return b

    def _setup_tray(self) -> None:
        self.tray = QSystemTrayIcon(dot_icon(AMBER), self)
        menu = QMenu()
        self.a_status = QAction("Checking…")
        self.a_status.setEnabled(False)
        menu.addAction(self.a_status)
        menu.addSeparator()
        for label, slot in (("Show window", self.show_window), ("Open dashboard", self.open_dashboard),
                            ("Start", self.start), ("Stop", self.stop), ("Restart", self.restart)):
            menu.addAction(label, slot)
        menu.addSeparator()
        menu.addAction("Quit widget", QApplication.instance().quit)
        self.tray.setContextMenu(menu)
        self.tray.activated.connect(
            lambda r: self.show_window() if r == QSystemTrayIcon.ActivationReason.Trigger else None)
        self.tray.show()

    def show_window(self) -> None:
        self.show()
        self.raise_()
        self.activateWindow()

    def log(self, text: str) -> None:
        self.out.appendPlainText(text.rstrip("\n"))

    # -- status -------------------------------------------------------------
    def refresh(self) -> None:
        self.running = port_open(PORT)
        pid = gateway_pid() if self.running else None
        if self.busy:
            color, text, detail = AMBER, "Working…", ""
        elif self.running:
            color, text = GREEN, "Running"
            detail = f"port {PORT}" + (f" · pid {pid} · up {uptime_of(pid)}" if pid else " · managed by systemd")
        else:
            color, text, detail = RED, "Stopped", f"nothing listening on port {PORT}"
        self.dot.setStyleSheet(f"color: {color}")
        self.state.setText(text)
        self.detail.setText(detail)
        self.tray.setIcon(dot_icon(color))
        self.tray.setToolTip(f"Kiro Crew: {text}")
        self.a_status.setText(f"Kiro Crew: {text}")
        idle = self.busy is None
        self.b_start.setEnabled(idle and not self.running)
        self.b_stop.setEnabled(idle and self.running)
        self.b_restart.setEnabled(idle and self.running)
        self.b_dash.setEnabled(self.running)
        for b in (self.b_logs, self.b_doctor):
            b.setEnabled(idle)
        for b in (self.b_cron, self.b_spawn):
            b.setEnabled(idle and self.running)

    # -- process plumbing ---------------------------------------------------
    def _proc(self, *args: str) -> QProcess:
        p = QProcess(self)
        env = QProcessEnvironment.systemEnvironment()
        p.setProcessEnvironment(env)
        p.setProgram(KIROCREW)
        p.setArguments(list(args))
        return p

    def run_cli(self, *args: str, then=None) -> None:
        if self.busy:
            return
        self.stop_follow()
        self.out.clear()
        self.log(f"$ kirocrew {' '.join(args)}")
        p = self._proc(*args)
        p.setProcessChannelMode(QProcess.ProcessChannelMode.MergedChannels)
        # The CLI prints config-deprecation WARNING noise on every call; drop it.
        p.readyReadStandardOutput.connect(lambda: self._drain(p))

        def done(code, _status):
            self._drain(p)
            self.busy = None
            if code:
                self.log(f"[exit {code}]")
            self.refresh()
            if then:
                then()
        p.finished.connect(done)
        self.busy = p
        p.start()
        self.refresh()

    def _drain(self, p: QProcess) -> None:
        for line in bytes(p.readAllStandardOutput()).decode(errors="replace").splitlines():
            if " WARNING kiro_crew.config.loader" in line:
                continue
            self.log(line)

    # -- actions ------------------------------------------------------------
    def start(self) -> None:
        if self.running or self.busy:
            return
        self.out.clear()
        self.log("Starting gateway…")
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
        t = QTimer(self, interval=500)
        self.busy = QProcess(self)  # placeholder marking "working"; never started

        def tick():
            if port_open(PORT) == up:
                t.stop(); self.busy = None; self.log(ok_msg); self.refresh()
            elif time.monotonic() > deadline:
                t.stop(); self.busy = None; self.log(fail_msg); self.refresh()
        t.timeout.connect(tick)
        t.start()
        self.refresh()

    def _confirm(self, what: str) -> bool:
        r = QMessageBox.question(self, "Kiro Crew", f"{what}\n\nThis interrupts running Kiro sessions. Continue?")
        return r == QMessageBox.StandardButton.Yes

    def stop(self) -> None:
        if self.running and not self.busy and self._confirm("Stop the gateway?"):
            self.run_cli("stop", then=lambda: self._wait_for(False, "Gateway stopped.", "Gateway still answering after stop."))

    def restart(self) -> None:
        if self.running and not self.busy and self._confirm("Restart the gateway?"):
            self.run_cli("restart", then=lambda: self._wait_for(True, "Gateway is back up.", "Gateway did not return within 30s."))

    def open_dashboard(self) -> None:
        # `kirocrew token` prints an authenticated URL; it is opened, never shown or logged.
        r = subprocess.run([KIROCREW, "token"], capture_output=True, text=True)
        url = next((l.strip() for l in r.stdout.splitlines() if l.strip().startswith("http")), f"http://localhost:{PORT}")
        QDesktopServices.openUrl(QUrl(url))

    def toggle_follow(self) -> None:
        if self.follower:
            self.stop_follow()
            return
        if self.busy:
            return
        self.out.clear()
        self.log("$ kirocrew logs -f")
        p = self._proc("logs", "-f", "-n", "50")
        p.setProcessChannelMode(QProcess.ProcessChannelMode.MergedChannels)
        p.readyReadStandardOutput.connect(lambda: self._drain(p))
        p.start()
        self.follower = p
        self.b_follow.setText("Stop following")

    def stop_follow(self) -> None:
        if self.follower:
            self.follower.kill()
            self.follower.deleteLater()
            self.follower = None
            self.b_follow.setText("Follow logs")

    def closeEvent(self, e) -> None:  # close to tray
        self.stop_follow()
        e.ignore()
        self.hide()


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationName("kiro-widget")
    app.setQuitOnLastWindowClosed(False)
    w = Widget()
    if "--hidden" not in sys.argv or not QSystemTrayIcon.isSystemTrayAvailable():
        w.show()
    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
