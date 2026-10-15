# Timer

A timer in the island: Pomodoro, cooking, coffee break. It is **off by
default**; turn it on in Settings › **Timer**.

## Using it

1. Open the island (hover or click): a **Timer** row offers your durations
   (5, 15 and 25 minutes by default).
2. Click one to start. The countdown shows, with **Pause** / **Resume** and
   **Reset**.
3. With the island closed, the pill shows "Timer · 12 min" when nothing more
   important needs your attention.
4. When it ends, the island grows ("Time's up!") and plays a sound. **OK**
   dismisses it; otherwise it fades on its own after 20 seconds.

The timer costs nothing at rest: it only wakes up every minute and at the end.

## Focus session

Next to the durations, **Focus 25 min** starts a focus session:

- **do not disturb** turns on: app notifications stop being announced (the
  moon lights up in the island);
- the pill shows "Focus · 18 min", with a thin progress line along its bottom
  edge;
- the mascot squints and stays still (it doesn't dance to music), then cheers
  at the end.

When the session ends, or if you reset it, do not disturb goes back to how it
was (if it was already on, it stays on). Claude Code still tells you when it is
waiting for you. Set the length in Settings › **Timer** (0: no button).

## Settings

```toml
[modules.timer]
enabled = true
presets = [5, 15, 25]   # 1 to 5 durations, in minutes (1 to 600)
focus_minutes = 25      # focus session (0: no button)
sound = true            # sound when it ends
done_secs = 20          # how long the "done" alert stays
```
