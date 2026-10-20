# kiro-widget

Tray widget for the Kiro Crew gateway (Rust, [`ksni`](https://crates.io/crates/ksni) over D-Bus). Click the tray dot for the menu.

- Live status (green = running, red = stopped, amber = working): port probe on 5476 (`KIROCREW_PORT` to override), PID and uptime
- **Start** / **Stop** (Stop asks first; it interrupts running Kiro sessions)
- **Go aboard** opens the dashboard (uses `kirocrew token`; the token URL is never displayed)
- Logs, Doctor, Cron jobs, Subagents (output opens in a `kdialog` text box)

Start and Stop go through `pkexec systemctl start|stop kirocrew.service` when that unit exists (the unit has `Restart=always`, so only an explicit `systemctl stop` keeps it down), else a detached `kirocrew gateway --no-open` / `kirocrew stop`.
Only one instance runs at a time (lock in `$XDG_RUNTIME_DIR`).

Needs `kdialog`, `notify-send` (libnotify), `xdg-open`, `pkexec` (polkit) and a StatusNotifier tray (KDE, or GNOME with an extension).

## Build and run

    cd rust && cargo build --release
    ./target/release/kiro-widget-rs

## Arch package

Local build from your checkout (committed `HEAD`):

    cd packaging/arch && makepkg -f
    sudo pacman -U kiro-widget-*.pkg.tar.zst

AUR recipe (`kiro-widget-git`, builds latest `main` from GitHub, no release versions) is in `packaging/aur/`.

Installs `/usr/bin/kiro-widget-rs`, a start-menu entry, and `/etc/xdg/autostart/kiro-widget-rs.desktop` so it launches at login.

## License

MIT, see `LICENSE`.
