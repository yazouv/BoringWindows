# Installation

## Download

BoringWindows is a single file. Go to the
**[Releases](https://github.com/yazouv/BoringWindows/releases/latest)** page and
download the one for your system:

| System | File |
|---|---|
| Windows 10/11 | `boringwindows-windows-x64.exe` |
| macOS (Apple Silicon) | `boringwindows-macos-arm64.tar.gz` |
| macOS (Intel) | `boringwindows-macos-x64.tar.gz` |
| Linux (x64) | `boringwindows-linux-x64.tar.gz` |

**Windows**: rename the file to `boringwindows.exe` if you like and put it
wherever you want, for instance in `C:\Users\<you>\Apps\BoringWindows\` (a
folder of yours: automatic updates must be able to replace it).

> Windows may show a SmartScreen warning ("unrecognized app") while the
> executable is not signed: *More info* › *Run anyway*.

**macOS**: extract the archive, then allow this unsigned binary:

```sh
xattr -d com.apple.quarantine boringwindows
./boringwindows
```

On a Mac, the island sits above the menu bar, centered at the top of the
screen (around the notch if there is one), and takes the height of the menu
bar while closed. Clicking the island doesn't take the focus away from the
current app, and clicks next to the pill go through. The icon lives in the
menu bar, not in the Dock. **Start with the system** adds a LaunchAgent
(`~/Library/LaunchAgents/io.github.yazouv.boringwindows.plist`). Not on Mac
yet: the blur (`theme.blur`), app notifications, battery, Bluetooth, volume
and presentation mode; and the island always hides in full-screen apps.

**Linux**: `tar -xzf boringwindows-linux-x64.tar.gz && ./boringwindows` (the
island opens as a floating window, without a tray icon).

## Updates

BoringWindows checks for new versions at startup, then every 6 hours. When it
finds one, it downloads it, verifies its checksum (SHA-256) and replaces its
executable; the island shows "BoringWindows x.y.z installed". It applies at
the next launch, or right away with right-click on the icon › **Restart to
update to x.y.z**.

- Turn off: **Settings… › General › Install updates automatically**
  (`auto_update = false` in `[general]`).
- Check by hand: **Settings… › General › Check for updates**, or the same entry
  in the icon menu.
- Only request sent: the list of releases from `api.github.com`, then the file
  download.
- Built from a **private** fork: GitHub only shows its releases to authorized
  accounts. Put a read-only token (*fine-grained token*, *Contents: Read*
  permission) in the `BORINGWINDOWS_GITHUB_TOKEN` environment variable. Not
  needed for the public repository.
- A version built with `cargo` doesn't update itself.

## Or build it yourself

You need [Rust](https://rustup.rs) and the Visual Studio C++ build tools
(offered by the Rust installer).

```powershell
git clone https://github.com/yazouv/BoringWindows
cd BoringWindows
cargo run --release
```

The first build takes a few minutes, the next ones a few seconds.

## First launch

Run `boringwindows.exe`: a black pill appears at the top centre of the screen,
and an icon is added to the notification area (bottom right, near the clock;
look under the ^ arrow if you don't see it).

On first launch, the configuration file is created with every option
commented:

```text
%APPDATA%\BoringWindows\config.toml
```

You don't need to change anything to start: music works right away. For Claude
Code and the calendar, follow their pages.

## Start with Windows

Right-click the icon › **Start with Windows**.

## Uninstall

1. If you connected Claude Code: right-click the icon › **Claude Code:
   remove hooks…**.
2. Untick **Start with Windows**, then **Quit**.
3. Delete `boringwindows.exe` and, if you want, the folders
   `%APPDATA%\BoringWindows` (configuration) and `%LOCALAPPDATA%\BoringWindows`
   (Claude relay, logs).
