# kiro-widget

Tray-only control panel for the Kiro Crew gateway; click the tray dot for the menu (PyQt6, wraps the `kirocrew` CLI).

- Live status (green/red/amber tray dot): port probe on 5476 (`KIROCREW_PORT` to override), PID and uptime
- Start / Stop (Stop asks first; it interrupts running Kiro sessions)
- "Go aboard" opens the dashboard (uses `kirocrew token`; the token URL is never displayed)
- Logs, Doctor, Cron jobs, Subagents (output opens in a small popup)

Start is `pkexec systemctl start kirocrew.service` when that unit exists, else a detached `kirocrew gateway --no-open`.

    python3 kiro_widget.py

Start menu: `cp kiro-widget.desktop ~/.local/share/applications/` (the Exec path is absolute; edit it if you move the repo).

Autostart: `cp kiro-widget.desktop ~/.config/autostart/`.
