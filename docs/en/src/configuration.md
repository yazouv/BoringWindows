# All options (config.toml)

The file lives in `%APPDATA%\BoringWindows\config.toml` (right-click the icon ›
**Ouvrir la configuration**). It is created on first launch with every option
commented. The common settings can also be changed without touching the
file: right-click › **Réglages…** (settings).

- **Save: it applies immediately**, no restart.
- A missing option takes its default value: you can keep only what you change.
- A typo (unknown option, out-of-range value) is reported in the island; the
  previous settings stay in place.

## Syntax reminder

```toml
# A comment
[section]                 # a section
option = "text"           # text in quotes
number = 42
switch = true             # true or false
list = ["a", "b"]

[[modules.calendar.sources]]   # double brackets: one list item, repeat it
name = "Work"
```

> Windows paths: write them with `/` (`"C:/Users/me/a.ics"`) or in single
> quotes (`'C:\Users\me\a.ics'`). With double quotes, `\` is interpreted and
> the file becomes invalid.

## `[general]`

| Option | Default | Values | Role |
|---|---|---|---|
| `monitor` | `"primary"` | `"primary"`, `"cursor"` | screen of the island: main one, or the one under the mouse at start up |
| `hide_in_fullscreen` | `true` | `true`, `false` | hide the island when an app is full screen |
| `open_on` | `"hover"` | `"hover"`, `"click"` | open the island on hover or on click |
| `collapse_delay_ms` | `350` | 0 to 10000 | delay before closing when the mouse leaves (ms) |

## `[theme]`

| Option | Default | Values | Role |
|---|---|---|---|
| `background` | `"#000000"` | `"#RRGGBB"` or `"#RRGGBBAA"` | island background |
| `foreground` | `"#FFFFFF"` | same | text |
| `accent` | `"#FF8A3D"` | same | accent colour (urgent, buttons); replaced by the artwork colour while music plays (see `[modules.media]`) |
| `corner_radius` | `22.0` | 0 to 500 | corner rounding of the open island |
| `animation_ms` | `240` | 0 to 2000 | animation duration, `0` for none |
| `top_offset` | `0.0` | 0 to 500 | offset from the top of the screen; above 0, the top corners are rounded too |

Sizes (in pixels, before Windows scaling):

| Section | Default | Role |
|---|---|---|
| `[theme.compact]` | `width = 190.0`, `height = 32.0` | idle pill |
| `[theme.attention]` | `width = 300.0`, `height = 36.0` | pill when a module has something to say |
| `[theme.expanded]` | `width = 520.0`, `height = 170.0` | open island; must be at least as big as the other two |

Width between 16 and 4000, height between 8 and 2000.

## `[layout]`

| Option | Default | Role |
|---|---|---|
| `compact` | `["claude", "media", "calendar"]` | priority order of modules **at equal importance** (something urgent always wins) |

## `[modules.claude]`

See [Claude Code](claude-code.md).

| Option | Default | Values | Role |
|---|---|---|---|
| `enabled` | `true` | | enable the module |
| `permissions` | `true` | | answer permission requests from the island |
| `permission_wait_secs` | `60` | 5 to 280 | time to answer in the island before handing back to the terminal (s) |
| `done_secs` | `8` | 0 to 600 | how long "terminé" (done) stays (s) |
| `sound` | `true` | | system sound when Claude starts waiting for you |

## `[modules.media]`

See [Music](musique.md).

| Option | Default | Role |
|---|---|---|
| `enabled` | `true` | enable the module |
| `accent_from_artwork` | `true` | tint the island with the artwork's dominant colour |
| `ignore` | `[]` | players to ignore, by part of their name: `["msedge", "chrome"]` |

## `[modules.calendar]`

See [Calendar](agenda/index.md).

| Option | Default | Values | Role |
|---|---|---|---|
| `enabled` | `true` | | enable the module |
| `remind_minutes` | `5` | 0 to 120 | reminder before an event starts (min) |
| `refresh_minutes` | `10` | 2 to 1440 | calendar download frequency (min) |
| `lookahead_hours` | `24` | 1 to 168 | horizon shown in the island (h) |
| `show_all_day` | `true` | | show all-day events |

Each calendar is a `[[modules.calendar.sources]]` block:

| Option | Required | Role |
|---|---|---|
| `name` | no | name used in messages |
| `url` | yes | `https://` or `webcal://` link, or path to an `.ics` file |

## `[modules.demo]`

| Option | Default | Role |
|---|---|---|
| `enabled` | `false` | demo module: a fake player and rotating alerts, to try the island without configuring anything |

## Full example

```toml
[general]
open_on = "hover"

[theme]
accent = "#5AC8FA"

[modules.calendar]
remind_minutes = 10
lookahead_hours = 48

[[modules.calendar.sources]]
name = "Work"
url = "https://outlook.office365.com/owa/calendar/…/calendar.ics"

[[modules.calendar.sources]]
name = "Classes"
url = "https://timetable.example.edu/calendar/G7a.ics"

[modules.media]
ignore = ["msedge"]
```
