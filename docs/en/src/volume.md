# Volume and brightness

Every change of the system volume (keyboard keys, Windows mixer, an
application) shows for a moment in the pill: "Volume 45 %" or "Muted". It is off
by default, since Windows already shows its own indicator.

Turn it on in Settings › **Notifications** › "Volume", or in `config.toml`:

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
- Windows only.

## Brightness

Same idea for the screen brightness: "Brightness 70 %" shows on every change
(keyboard keys, notification center, battery saver). It is off by default too.

Turn it on in Settings › **Notifications** › "Brightness", or in
`config.toml`:

```toml
[modules.brightness]
enabled = true
show_secs = 2   # how long it shows after a change (1 to 10)
```

- Only the **built-in screen** of a laptop or tablet reports its brightness to
  Windows. On a desktop PC or an external monitor, the module does nothing (the
  log says no built-in screen was found).
- It listens to a WMI event (`WmiMonitorBrightnessEvent`): its dedicated thread
  wakes at most every 2 seconds to check whether the module should stop, with no
  other cost at rest.
