# CPU, memory and shortcuts

## CPU and memory

In the open island, next to the time (and the weather), two small counters
show the **CPU** and **memory** use. **Hover them**: the date gives way to
the memory in use, for example "Memory 8.4 / 16.0 GB". A value above
90 % turns red.

Nothing is measured while the island is closed: measuring only runs while it
is open (every 2 seconds by default).

### Alerts

You can be warned when the CPU or the memory **stays** busy: after 30
seconds above the threshold, the pill shows for example "CPU at 97 % ·
chrome", with the app using the most. The alert only comes back once the load
has clearly gone down (10 points below the threshold). With an alert set, a
measure is taken every 5 seconds, even while the island is closed.

Settings › **System**: tick or untick the display, pick the refresh rate and
the alert thresholds. Or in `config.toml`:

```toml
[modules.system]
enabled = true
refresh_secs = 2          # 1 to 10, while the island is open
cpu_alert_percent = 90    # 0 = no alert, otherwise 50 to 100
ram_alert_percent = 0
alert_after_secs = 30     # 5 to 600
```

## Keyboard shortcuts

Shortcuts that work in every app:

| Action | Config key | Default |
|---|---|---|
| Open or close the island | `toggle` | `Ctrl+Alt+B` |
| Play / pause | `play_pause` | none |
| Next track | `next_track` | none |
| Previous track | `previous_track` | none |
| Do not disturb | `do_not_disturb` | none |

When opened with the keyboard, the island closes by itself after 6 seconds if
the mouse doesn't come over it (or on the next press of the shortcut).

Settings › **System** › *Keyboard shortcuts*, or in `config.toml`:

```toml
[hotkeys]
toggle = "Ctrl+Alt+B"
play_pause = "Ctrl+Alt+P"
next_track = "Ctrl+Alt+Right"
previous_track = "Ctrl+Alt+Left"
do_not_disturb = ""          # empty: no shortcut
```

Write a shortcut with its modifiers first, separated by `+`:

- **Modifiers**: `Ctrl`, `Alt`, `Shift`, `Super` (Windows key; `Cmd` on a
  Mac). At least one is required, except for `F13` to `F24`, `Pause` and
  `ScrollLock`.
- **Keys**: `A` to `Z`, `0` to `9`, `F1` to `F24`, `Space`, `Enter`, `Tab`,
  `Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PageUp`, `PageDown`,
  `Insert`, `Delete`, `Num0` to `Num9`…

If a shortcut is already taken by another app, or badly written, the island
says so (`⚠ …`) and the other shortcuts keep working.

> On Linux, shortcuts go through X11: they don't work in a pure Wayland
> session.
