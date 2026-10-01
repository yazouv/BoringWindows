# Installation

## Download

BoringWindows is a single file, `boringwindows.exe`.

1. Open the **[Actions](https://github.com/yazouv/BoringWindows/actions)** tab
   of the repository and click the latest successful run (✅).
2. At the bottom of the page, under **Artifacts**, download
   `boringwindows-windows-x64` (you need to be signed in to GitHub), then
   unzip it.
3. Put `boringwindows.exe` wherever you like, for instance in
   `C:\Users\<you>\Apps\BoringWindows\`.

> Windows may show a SmartScreen warning ("unrecognized app") while the
> executable is not signed: *More info* › *Run anyway*.

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

Right-click the icon › **Lancer au démarrage** (start with Windows).

## Uninstall

1. If you connected Claude Code: right-click the icon › **Claude Code :
   retirer les hooks…** (remove hooks).
2. Untick **Lancer au démarrage**, then **Quitter** (quit).
3. Delete `boringwindows.exe` and, if you want, the folders
   `%APPDATA%\BoringWindows` (configuration) and `%LOCALAPPDATA%\BoringWindows`
   (Claude relay, logs).
