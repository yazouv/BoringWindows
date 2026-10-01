# Volume

Every change of the system volume (keyboard keys, Windows mixer, an
application) shows for a moment in the pill: "Volume 45 %" or "Muted". It is off
by default, since Windows already shows its own indicator.

Turn it on in Settings › **General** › "Show volume changes in the island", or
in `config.toml`:

```toml
[modules.volume]
enabled = true
show_secs = 2   # how long it shows after a change (1 to 10)
```

What it does, and does not do:

- The module listens to the **default** output device; it costs nothing at rest
  (a Windows audio API callback, no polling).
- It does **not** remove Windows' own indicator: both show.
- If you change the default output device on the fly, restart BoringWindows (or
  reload the modules by editing `config.toml`).
- Windows only. Brightness is not handled.
