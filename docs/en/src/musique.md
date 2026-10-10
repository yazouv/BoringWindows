# Music

**Nothing to set up.** BoringWindows reads the same information as the Windows
volume overlay: any player that shows up there shows up in the island too.
Spotify, Apple Music, Deezer, Tidal, YouTube and SoundCloud in the browser,
VLC, the Windows media player… no account or API key.

![The player in the open island, then the artwork in the pill](images/musique.png)

## What you see

- **Pill**: the artwork and `♪ title — artist` while music is playing.
- **Open island**: artwork, title, artist, **previous / play / next** buttons
  and a progress bar. **Click the bar** to seek within the track.
- **Colour**: the island takes the dominant colour of the artwork (buttons,
  bar), brightened to stay readable on black.

A greyed-out button means the player doesn't offer that action (some websites
don't allow seeking, for example).

## Several players

If several players are open, **the one that is playing** is shown. At the top
right of the island, the source name is followed by `⇄`: click it to switch to
the next player.

To never show a player (your work browser, for example):

```toml
[modules.media]
ignore = ["msedge"]
```

Part of the name is enough: `"chrome"`, `"msedge"`, `"firefox"`, `"spotify"`…
The exact names are printed in the console (`musique : sources [...]`) when
you start BoringWindows from a terminal.

## On a Mac

macOS has no public equivalent of the Windows media controls: there,
BoringWindows follows **Spotify** and **Music** (Apple Music). They announce
every track change, play or pause; the island then reads the track with
AppleScript. On first use, macOS asks whether BoringWindows (or your
terminal) may control the app: accept, otherwise nothing shows (System
Settings › Privacy & Security › Automation to change it later). Browsers and
other players aren't supported yet.

## Settings

```toml
[modules.media]
enabled = true               # false to turn it off completely
accent_from_artwork = true   # tint the island with the artwork
ignore = []
```

## Audio visualizer (optional)

Small animated equalizer bars in the island's header, driven by the sound the
system actually plays. This is the **only feature that works continuously**, so
it is off by default; capture only runs while the island is open **and** music
is playing, and stops as soon as either stops being true.

Turn it on in Settings › **Music**, or in `config.toml`:

```toml
[modules.visualizer]
enabled = true
bands = 12   # number of bars (4 to 32)
fps = 30     # frames per second (10 to 60)
```

Windows only (loopback capture of the default output device). It picks up
everything the system plays, not only the player shown.
