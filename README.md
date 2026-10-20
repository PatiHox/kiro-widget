# kiro-widget

Tray icon + small window for the Kiro Crew gateway (PyQt6, wraps the `kirocrew` CLI).

- Live status (green/red/amber tray dot): port probe on 5476 (`KIROCREW_PORT` to override), PID and uptime
- Start / Stop / Restart (stop and restart ask first; they interrupt running Kiro sessions)
- Open dashboard (uses `kirocrew token`; the token URL is never displayed)
- Logs, live log follow, Doctor, Cron jobs, Subagents

Start is `pkexec systemctl start kirocrew.service` when that unit exists, else a detached `kirocrew gateway --no-open`.

    python3 kiro_widget.py            # window + tray
    python3 kiro_widget.py --hidden   # tray only

Autostart: `cp kiro-widget.desktop ~/.config/autostart/` (add `--hidden` to Exec for tray-only).
