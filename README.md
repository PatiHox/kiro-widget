# kiro-widget

Tray widget for the Kiro Crew gateway (Rust, [`ksni`](https://crates.io/crates/ksni) over D-Bus). Click the tray dot for the menu.

- Live status (green = running, red = stopped, amber = working): port probe on 5476 (`KIROCREW_PORT` to override), PID and uptime
- **Start** / **Stop** (Stop asks first; it interrupts running Kiro sessions)
- **Go aboard** opens the dashboard (uses `kirocrew token`; the token URL is never displayed)
- Logs, Doctor, Cron jobs, Subagents (output opens in a dialog)

Start and Stop go through `pkexec systemctl start|stop kirocrew.service` when that unit exists (the unit has `Restart=always`, so only an explicit `systemctl stop` keeps it down), else a detached `kirocrew gateway --no-open` / `kirocrew stop`.
Only one instance runs at a time (lock in `$XDG_RUNTIME_DIR`).

Needs `zenity` or `kdialog` (dialogs), `notify-send` (libnotify), `xdg-open`, `pkexec` (polkit, with an authentication agent running) and a StatusNotifier tray host (see below).

## How this relates to the official KiroCrew desktop app

Kiro Crew ships its own desktop app (Electron; `.deb`, `.rpm` and AppImage on Linux, a `.dmg` on macOS). This widget is not a replacement for it. It is for the setup the app does not cover: the gateway running as a service (`kirocrew service install`, systemd) with the dashboard in a browser tab, and no Electron app installed.

What the widget adds over the app:

- A live status dot. The app's tray icon is static (Show, New Connection Window, Open Config File, Quit) and shows no gateway state.
- Start and Stop for a systemd-managed gateway. The app adopts such a gateway but never stops or evicts it.
- A small Rust binary instead of Electron, with an XDG autostart entry.

If you use the desktop app, you do not need the widget. The app starts the gateway when it launches and stops it on quit, has the dashboard in its window and mints the token itself. Logs, cron jobs and subagents are dashboard features. Doctor is the CLI command `kirocrew doctor`.

The widget is Linux-only (StatusNotifier over D-Bus, `systemctl`, `pkexec`, `/proc`). Kiro Crew itself supports macOS, but there the signed app covers the same ground, so there is no macOS port. Someone running only the launchd LaunchAgent (`kirocrew service install`) with no app would be the one case it leaves open.

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
