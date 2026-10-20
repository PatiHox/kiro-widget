# kiro-widget

Tray widget for the Kiro Crew gateway (Rust, [`ksni`](https://crates.io/crates/ksni) over D-Bus). Click the tray ghost for the menu.

- Live status (the ghost is Kiro purple = running, amber = working, red with its eyes shut = stopped): port probe on 5476 (`KIROCREW_PORT` to override), PID and uptime
- **Start** / **Stop** (Stop asks first; it interrupts running Kiro sessions)
- **Go aboard** opens the dashboard (uses `kirocrew token`; the token URL is never displayed)
- Logs, Doctor, Cron jobs, Subagents (output opens in a dialog)

Start and Stop go through `pkexec systemctl start|stop kirocrew.service` when that unit exists (the unit has `Restart=always`, so only an explicit `systemctl stop` keeps it down), else a detached `kirocrew gateway --no-open` / `kirocrew stop`.
Only one instance runs at a time (lock in `$XDG_RUNTIME_DIR`).

Needs `zenity` or `kdialog` (dialogs), `notify-send` (libnotify), `xdg-open`, `pkexec` (polkit, with an authentication agent running) and a StatusNotifier tray host (see below).

## Supported desktops

The tray is a StatusNotifierItem over the D-Bus session bus (`dbus-broker` and `dbus-daemon` both work), so X11 and Wayland behave the same. What differs is the tray host:

| Desktop | Tray host | Dialogs | Status |
|---|---|---|---|
| KDE Plasma | built in | `kdialog` | tested (Wayland) |
| GNOME | [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/) (`gnome-shell-extension-appindicator`); preinstalled on Ubuntu | `zenity` | not tested |
| Xfce 4.16+ | built-in "Status Tray Plugin" | `zenity` | not tested |
| Cinnamon | `xapp` (`xapp-sn-watcher`) + the XApp Status applet | `zenity` | not tested |

If the tray host starts after the widget (common at login), the widget waits and registers when it appears. If none shows up within 30 seconds it sends a desktop-specific hint. Bare X11 window managers with an XEmbed-only tray need a proxy such as `snixembed`.

What is verified: the no-host and late-host cases and the dialog command shapes were exercised on a private D-Bus session, and dialog selection has unit tests. The GNOME, Xfce and Cinnamon rows have not been run on those desktops.

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

## Credits

The tray ghost (`rust/assets/ghost*.svg`, adapted) is the "ghost" icon from [Scarlab Icons](https://scarlab-icons.la-moore.ru) by Alex Moore (MIT), found on [SVG Repo](https://www.svgrepo.com/svg/508069/ghost). The notice is in `THIRD_PARTY_NOTICES.md`.
