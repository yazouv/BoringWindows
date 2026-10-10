# Gestures

A few gestures on the island, to go fast:

| Gesture | Effect |
|---|---|
| Wheel up / down | volume up / down (2 % per notch) |
| Swipe left on the touchpad, wheel tilted right, or Shift + wheel | next track |
| Swipe right (or the other way) | previous track |
| Click and drag horizontally on the island with the mouse | next or previous track |
| Long press (a bit more than half a second) | turn "do not disturb" on or off |

The island confirms each gesture: "Volume 46 %", "Next track"… in the header
when it is open, in the pill otherwise. When the [volume](volume.md) module is
on, its own display shows the volume.

Settings › **General** › "Gestures", or in `config.toml`:

```toml
[modules.gestures]
enabled = true
volume_step = 2   # % per wheel notch (1 to 10)
```

Volume is Windows only; tracks go through the media controls, like the player
buttons.
