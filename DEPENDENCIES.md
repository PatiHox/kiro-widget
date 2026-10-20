# Installing the system dependencies

What kiro-widget needs at runtime, and what you need to build it, as install commands for the common package managers. See [Requirements](README.md#requirements) in the README for what each tool is used for.

The `.deb`, `.rpm` and Arch packages pull in the runtime tools themselves. For a source build, or for what no package covers, install them with your package manager. In each block the first line is the runtime tools; the second is the build tools, needed only to build from source.

## Debian / Ubuntu

    sudo apt install zenity libnotify-bin xdg-utils pkexec
    sudo apt install build-essential curl

On a release without a `pkexec` package, install `policykit-1` instead.

## Fedora

    sudo dnf install zenity libnotify xdg-utils polkit
    sudo dnf install gcc curl

## Arch Linux

    sudo pacman -S zenity libnotify xdg-utils polkit
    sudo pacman -S base-devel curl

## openSUSE

    sudo zypper install zenity libnotify-tools xdg-utils polkit
    sudo zypper install gcc curl

## NixOS

In `configuration.nix`. The `pkexec` that Nix installs is not setuid, so enable polkit through the module to get the wrapper:

    security.polkit.enable = true;
    environment.systemPackages = with pkgs; [ zenity libnotify xdg-utils ];

## Optional extras

By package name:

| | `kdialog` (native dialogs on KDE) | AppIndicator tray (GNOME) |
|---|---|---|
| apt | `kdialog` | `gnome-shell-extension-appindicator` |
| dnf | `kdialog` | `gnome-shell-extension-appindicator` |
| pacman | `kdialog` | `gnome-shell-extension-appindicator` |
| zypper | `kdialog` | `gnome-shell-extension-appindicator` |
| Nix | `kdePackages.kdialog` | `gnomeExtensions.appindicator` |

After installing the GNOME extension, enable it in the Extensions app and log out and back in. Ubuntu preinstalls it. Tray hosts for Xfce and Cinnamon are listed under [Supported desktops](README.md#supported-desktops).

## Rust

Rust itself comes from [rustup](https://rustup.rs), since distro `rustc`/`cargo` packages are often older than the 1.89 required (Ubuntu 24.04 ships 1.75):

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
