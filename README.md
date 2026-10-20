# kiro-widget

Tray widget for the Kiro Crew gateway (Rust, [`ksni`](https://crates.io/crates/ksni) over D-Bus). Click the tray dot for the menu.

- Live status (Kiro purple = running, red = stopped, amber = working): port probe on 5476 (`KIROCREW_PORT` to override), PID and uptime
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

## Debian / Fedora packages

`.deb` and `.rpm` are built from `rust/Cargo.toml` metadata with [`cargo-deb`](https://crates.io/crates/cargo-deb) and [`cargo-generate-rpm`](https://crates.io/crates/cargo-generate-rpm):

    cd rust && cargo build --release
    cargo deb --no-build          # target/debian/kiro-widget_*.deb
    cargo generate-rpm            # target/generate-rpm/kiro-widget-*.rpm

CI (`.github/workflows/ci.yml`) runs fmt, clippy, the build and both packagers on every push and PR and uploads the packages as artifacts. Pushing a `v*` tag also attaches them to a GitHub release. Dependabot (`.github/dependabot.yml`) opens weekly grouped updates for Cargo dependencies and GitHub Actions.

## License

MIT, see `LICENSE`.
