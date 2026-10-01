# Using the island

## The three states

| State | When | What you see |
|---|---|---|
| **Compact** | nothing to report | a small black pill |
| **Attention** | a module has something to say | a wider pill with a dot and a short text |
| **Open** | mouse over the island (or click) | the time, music, calendar, Claude sessions |

![Compact, attention (music), urgent attention (Claude), open](images/etats.png)

The pill's dot is **orange** when it's urgent (Claude is waiting for an
answer), grey otherwise. When music has the floor, the artwork replaces the
dot.

When several things happen at once, the most important one wins: a question
from Claude comes before a meeting that is starting, which comes before music.
The order between items of equal importance is set with
[`layout.compact`](configuration.md#layout).

## Open and close

- **Hover** the island to open it; it closes when the mouse leaves.
- Prefer clicking? Set `open_on = "click"` in
  [`[general]`](configuration.md#general).
- Only the pill reacts to the mouse: the rest of the strip at the top of the
  screen lets clicks through to the windows below.
- The island never takes focus: your keyboard stays in the app you're typing
  in.

## The tray menu

Right-click the BoringWindows icon in the notification area (the menu is in
French for now):

| Entry | Effect |
|---|---|
| Réglages… | opens the settings window (see below) |
| Ouvrir la configuration | opens `config.toml` in your editor |
| Recharger la configuration | re-reads the file (only needed if automatic reload failed) |
| Claude Code : installer / retirer les hooks… | connects Claude Code (see [Claude Code](claude-code.md)) |
| Lancer au démarrage | starts BoringWindows with Windows |
| Masquer l'île | hides the island without quitting |
| Quitter | quits BoringWindows |

## Full screen

When an application goes full screen (game, F11 video, presentation) on the
same screen, the island hides, then comes back when full screen ends. Turn this
off with `hide_in_fullscreen = false`.

## Changing the configuration

The easiest way: right-click the icon › **Réglages…** (settings). A tabbed
window (Général, Apparence, Agenda, Claude Code, Musique — general, appearance,
calendar, Claude Code, music) edits the common settings; **every change is
saved and applied immediately**, there is no "OK" button. That's also where you
add a calendar (per-service assistant, **Tester** button), install the Claude
Code hooks and run the diagnostic. The window can also open at startup:
`boringwindows.exe --settings`. Like the menu, it is in French for now.

The window writes to `config.toml` and keeps your comments and the order of
the file. For everything else (island sizes, module order…), edit the file
directly.

Everything is set in `%APPDATA%\BoringWindows\config.toml`. **Save the file: the
island updates immediately**, no restart needed. If the file contains an error,
the island shows it ("⚠ config.toml invalide…") and keeps the previous
settings.

Every option is described in the [reference](configuration.md).
