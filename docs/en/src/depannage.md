# Troubleshooting

## The island doesn't show up

- Check whether the icon is in the notification area (^ arrow). If so, make
  sure **Hide the island** isn't ticked.
- Is an app full screen on that screen? The island then hides on purpose
  (`hide_in_fullscreen`).
- Several screens: the island goes to the **main** screen by default
  (`monitor = "cursor"` for the mouse's screen).
- BoringWindows only runs once: a second launch does nothing if the first one
  is already running.

## "⚠ invalid config.toml"

The message gives the line and the problem. Most common causes:

- **a Windows path in double quotes**: `"C:\Users\…"` → write `"C:/Users/…"`
  or `'C:\Users\…'`;
- **a misspelt option**: names are checked, so a typo isn't silently ignored;
- **an out-of-range value**: see the [reference](configuration.md);
- "smart" quotes pasted from a word processor: use `"` or `'`.

Fix it and save: the island goes back to normal right away.

## Claude Code: nothing shows up

1. Did you install the hooks (right-click › **Claude Code: install
   hooks…**) **and restart** your Claude Code sessions since?
2. Run the diagnostic, with BoringWindows running, in another terminal:

   ```powershell
   boringwindows.exe doctor
   ```

   It checks, in order:

   | Step | If `[!!]` |
   |---|---|
   | 1. Hooks in `settings.json` | install the hooks from the menu |
   | 2. Relay | restart BoringWindows (it copies the relay again) |
   | 3. BoringWindows running | start BoringWindows before the diagnostic |
   | 4. Relay run like Claude Code | send the report in an issue |
   | 5. Log | shows Claude Code's latest real calls |
   | 6. Visual test | "diagnostic" must show for 6 s in the island |

   The report is also saved to `%LOCALAPPDATA%\BoringWindows\doctor.txt`.
3. The relay log (`%LOCALAPPDATA%\BoringWindows\hook.log`) has one line per
   event received from Claude Code: if it's empty, Claude doesn't call the
   relay (hooks not reloaded yet: restart Claude).

## Claude Code: a request stays on screen

If you pressed Escape on a request, it disappears with the next message you
send to Claude, or after the delay (60 s by default).

## Music: a player doesn't show up

BoringWindows shows what Windows shows in its volume overlay (media keys). If
the player isn't there either, the player doesn't publish its information:
some players have an option like "Integrate with Windows media controls". Also
check it isn't in `ignore`.

## Calendar: nothing shows up

- Start BoringWindows from a terminal: the console prints
  `agenda : Work — 12 événement(s) à venir` (12 upcoming events), or the
  error.
- **0 events**: the calendar may be empty for the next 24 h
  (`lookahead_hours = 48` to see further), or the link only publishes
  free/busy (Outlook: choose "Can view all details").
- **download failed**: paste the link in your browser; if it downloads an
  `.ics` file, the link is fine. Otherwise regenerate it from your service.
- Recent changes may take a while to appear: Google and Outlook don't update
  their ICS link instantly.

## Reporting a problem

Open an [issue](https://github.com/yazouv/BoringWindows/issues) with what you
did, what you expected, what happened, and if possible the console lines or
the `doctor` report.
