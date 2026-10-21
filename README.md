# kiro-widget

Tray widget for the Kiro Crew gateway (Rust, [`ksni`](https://crates.io/crates/ksni) over D-Bus). Click the tray ghost for the menu.

- Live status (the ghost is Kiro purple = running, amber = working, red with its eyes shut = stopped): port probe on 5476 (`KIROCREW_PORT` to override), PID and uptime
- **Start** / **Stop** (Stop asks first; it interrupts running Kiro sessions)
- **Go aboard** opens the dashboard (uses `kirocrew token`; the token URL is never displayed)
- Logs, Doctor, Cron jobs, Subagents (output opens in a dialog)
- Lightweight: a 2.3 MB binary that idles at about 5 MB of RAM

Start and Stop go through `pkexec systemctl start|stop kirocrew.service` when that unit exists (the unit has `Restart=always`, so only an explicit `systemctl stop` keeps it down), else a detached `kirocrew gateway --no-open` / `kirocrew stop`.
Only one instance runs at a time (lock in `$XDG_RUNTIME_DIR`).

## Requirements

**Runtime**

- Linux with a D-Bus session bus. The widget is Linux-only (StatusNotifier over D-Bus, `systemctl`, `pkexec`, `/proc`).
- Kiro Crew (the `kirocrew` CLI), installed separately. The widget looks for it on `PATH`, then in `~/.local/bin/kirocrew`.
- A StatusNotifier tray host. Built in on KDE Plasma. GNOME needs the AppIndicator extension (`gnome-shell-extension-appindicator`; Nix: `gnomeExtensions.appindicator`), which you enable in the Extensions app before logging out and back in (Ubuntu preinstalls it). Other desktops: see [Supported desktops](#supported-desktops).
- `zenity` or `kdialog` for dialogs (`kdialog` is the native choice on KDE; Nix: `kdePackages.kdialog`).
- `notify-send` (`libnotify`) for notifications.
- `xdg-open` (`xdg-utils`) to open the dashboard.
- `pkexec` (`polkit`) with an authentication agent running, for Start and Stop.
- `systemd` is optional: it is only needed to manage the `kirocrew.service` unit. Without it the widget falls back to a detached `kirocrew gateway --no-open` / `kirocrew stop`.

**Building from source** (the From source route and building packages yourself; the Arch recipes pull in `cargo` on their own)

- Rust 1.89 or newer (stable) with `cargo`. The crate uses edition 2024 (Rust 1.85+) and the locked dependencies need 1.89. CI builds on the latest stable. Distro `rustc` is often older (Ubuntu 24.04 ships 1.75), so use [rustup](https://rustup.rs).
- A C linker (`gcc` or `cc`).
- Optional, to build packages: [`cargo-deb`](https://crates.io/crates/cargo-deb), [`cargo-generate-rpm`](https://crates.io/crates/cargo-generate-rpm), or `makepkg` (`base-devel`) on Arch.

## Installation

Pick one. All of them install `kiro-widget-rs`; the packages also add a start-menu entry and an `/etc/xdg/autostart/kiro-widget-rs.desktop` entry so it launches at login.

The packages pull in the runtime tools they can express: `zenity` (or `kdialog` on Debian), `libnotify`, `polkit` and `xdg-utils`. The tray host and `kirocrew` are not packaged dependencies, so install those yourself (see [Requirements](#requirements)).

### Arch Linux

AUR recipe (`kiro-widget-git`, builds latest `main` from GitHub, no release versions) is in `packaging/aur/`:

    git clone https://github.com/PatiHox/kiro-widget.git
    cd kiro-widget/packaging/aur && makepkg -si

Or a local build from your checkout (committed `HEAD`):

    cd packaging/arch && makepkg -f
    sudo pacman -U kiro-widget-*.pkg.tar.zst

### Debian / Ubuntu

Download the `.deb` (attached to a GitHub release when a `v*` tag is pushed, and to every CI run as an artifact), or [build it](#building-packages), then:

    sudo apt install ./kiro-widget_*.deb

### Fedora

    sudo dnf install ./kiro-widget-*.rpm

### From source

Install the dependencies first. In each pair the first line is the runtime tools and the second is the build tools:

    # Debian / Ubuntu (no pkexec package on your release? use policykit-1)
    sudo apt install zenity libnotify-bin xdg-utils pkexec
    sudo apt install build-essential curl

    # Fedora
    sudo dnf install zenity libnotify xdg-utils polkit
    sudo dnf install gcc curl

    # Arch Linux
    sudo pacman -S zenity libnotify xdg-utils polkit
    sudo pacman -S base-devel curl

    # openSUSE
    sudo zypper install zenity libnotify-tools xdg-utils polkit
    sudo zypper install gcc curl

    # NixOS, in configuration.nix (runtime tools only; the module provides the setuid pkexec wrapper)
    security.polkit.enable = true;
    environment.systemPackages = with pkgs; [ zenity libnotify xdg-utils ];

Then Rust:

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Then build and run:

    git clone https://github.com/PatiHox/kiro-widget.git
    cd kiro-widget/rust
    cargo build --release
    ./target/release/kiro-widget-rs

This runs the widget in place without installing it. To install it for your user and start it at login:

    install -Dm755 target/release/kiro-widget-rs ~/.local/bin/kiro-widget-rs
    install -Dm644 ../packaging/kiro-widget-rs.desktop ~/.config/autostart/kiro-widget-rs.desktop

The `.desktop` file starts `kiro-widget-rs` from `PATH`, so make sure `~/.local/bin` is on it in your session.

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

## Building packages

`.deb` and `.rpm` are built from `rust/Cargo.toml` metadata with [`cargo-deb`](https://crates.io/crates/cargo-deb) and [`cargo-generate-rpm`](https://crates.io/crates/cargo-generate-rpm):

    cd rust && cargo build --release
    cargo deb --no-build          # target/debian/kiro-widget_*.deb
    cargo generate-rpm            # target/generate-rpm/kiro-widget-*.rpm

CI (`.github/workflows/ci.yml`) runs fmt, clippy, the build and both packagers on every push and PR and uploads the packages as artifacts. Pushing a `v*` tag also attaches them to a GitHub release. Dependabot (`.github/dependabot.yml`) opens weekly grouped updates for Cargo dependencies and GitHub Actions.

## Versioning and releases

Versions follow [Semantic Versioning](https://semver.org) and start at 0.0.1. Notable changes are logged in [CHANGELOG.md](CHANGELOG.md), in [Keep a Changelog](https://keepachangelog.com) format.

The public interface is the tray menu and its behaviour, the environment variables (`KIROCREW_PORT`), the `kirocrew` commands the widget calls, and what the packages install (binary name, desktop and autostart entries).

- **MAJOR**: a breaking change to that interface, such as removing or renaming an environment variable, renaming the binary or its installed paths, or dropping a supported desktop or `kirocrew` version.
- **MINOR**: new menu entries or features, a newly supported desktop, a raised minimum Rust version.
- **PATCH**: bug fixes, dependency updates, packaging and docs fixes.

Before 1.0.0 (`0.y.z`) the interface may change at any time: a breaking change bumps MINOR, everything else bumps PATCH. 1.0.0 is cut once the interface above is considered stable.

To cut a release:

1. Add a changelog line under `[Unreleased]` with every user-visible change as you make it. For a release, move those lines under a new `## [X.Y.Z] - YYYY-MM-DD` heading and update the links at the bottom of `CHANGELOG.md`.
2. Set `version` in `rust/Cargo.toml` and refresh the lock file with `cargo check` in `rust/`.
3. Commit, then `git tag vX.Y.Z && git push origin vX.Y.Z`. CI attaches the `.deb` and `.rpm` to a GitHub release.

The `.deb` and `.rpm` take their version from `rust/Cargo.toml`, not from the tag, so keep the two in step. The Arch and AUR packages version themselves from git (`r<commit count>.<hash>`) and ignore this number.

## License

MIT, see `LICENSE`.

## Credits

The tray ghost (`rust/assets/ghost*.svg`, adapted) is the "ghost" icon from [Scarlab Icons](https://scarlab-icons.la-moore.ru) by Alex Moore (MIT), found on [SVG Repo](https://www.svgrepo.com/svg/508069/ghost). The notice is in `THIRD_PARTY_NOTICES.md`.
