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

## Rust port

`rust/` is a feature-for-feature port using [`ksni`](https://crates.io/crates/ksni) (StatusNotifierItem over D-Bus), with `kdialog` for the confirm/output dialogs and `notify-send` for notifications. No GUI toolkit.

    cd rust && cargo build --release
    ./target/release/kiro-widget-rs

Measured on this machine, idle: Python/PyQt6 ~72 MB resident, Rust ~6 MB. Use one or the other, not both (two tray icons).
`rust/kiro-widget-rs.desktop` is the matching start-menu entry (absolute path to the release binary).

## Arch package

    cd packaging/arch && makepkg -f      # builds from committed HEAD
    sudo pacman -U kiro-widget-*.pkg.tar.zst

Installs `/usr/bin/kiro-widget-rs` and a start-menu entry. For autostart: `cp /usr/share/applications/kiro-widget-rs.desktop ~/.config/autostart/`.
