# Custom views (.slint)

You can replace the content of the open island with your own view, written in
[Slint](https://slint.dev): a plain text file, reloaded live as soon as you save
it, without rebuilding BoringWindows.

## Setting it up

1. Settings › **Appearance** › **Layouts folder** (or create
   `%APPDATA%\BoringWindows\layouts\` by hand).
2. Write `layouts\my-view.slint` (example below).
3. Settings › **Custom view of the open island** › `my-view`, or in
   `config.toml`:

```toml
[layout]
view = "my-view"   # file layouts/my-view.slint ; empty = built-in view
```

An error in the file shows in the island (`⚠ my-view.slint:12 …`) and the
built-in view stays until you fix it.

## The file

It exports a `View` component inheriting `Window`. It fills the open island
(sized by `[theme.expanded]`). Standard widgets
(`import { Button } from "std-widgets.slint";`, Fluent style) are available, and
so are the other `.slint` files of the same folder (`import "other.slint";`).

Just **declare** the properties and callbacks you need: the ones you don't
declare are ignored.

## Data (`in property`)

| Property | Type | Content |
|---|---|---|
| `expanded` | bool | the island is open |
| `time-text`, `date-text` | string | time and date |
| `accent`, `foreground`, `background` | color | theme colors (the accent follows the album art) |
| `has-media` | bool | music detected |
| `media-title`, `media-artist`, `media-source` | string | current track |
| `media-playing` | bool | playing |
| `media-position`, `media-duration` | string | "1:23", "3:45" |
| `media-progress` | float | 0 to 1 |
| `has-media-art`, `media-art` | bool, image | album art |
| `media-multi-source`, `media-can-previous`, `media-can-next`, `media-can-toggle`, `media-can-seek` | bool | what the player allows |
| `has-prompt`, `prompt-id`, `prompt-project`, `prompt-tool`, `prompt-detail` | bool, string | Claude Code permission request |
| `claude-rows` | `[{ id, project, status: string, urgent, active: bool }]` | Claude Code sessions |
| `agenda-rows` | `[{ title, time, location, relative, join-url: string, has-join, soon: bool }]` | upcoming events |
| `has-timer`, `timer-phase`, `timer-time`, `timer-progress`, `timer-presets` | bool, int, string, float, `[string]` | timer (phase: 0 ready, 1 running, 2 paused, 3 done) |

## Actions (callbacks)

| Callback | Effect |
|---|---|
| `media-action(string)` | `"prev"`, `"toggle"`, `"next"`, `"source"` |
| `media-seek(float)` | seek in the track (0 to 1) |
| `claude-decide(string, string)` | `(prompt-id, "allow" \| "deny" \| "ask")` |
| `claude-focus(string)` | brings the session's terminal to the front (`id`) |
| `open-url(string)` | opens an `https://` link (e.g. `join-url`) |
| `timer-action(string)` | `"start:<minutes>"`, `"pause"`, `"resume"`, `"reset"` |

## Example

```slint
export component View inherits Window {
    in property <string> time-text;
    in property <bool> has-media;
    in property <string> media-title;
    in property <string> media-artist;
    in property <float> media-progress;
    in property <color> accent;
    in property <[{ title: string, time: string }]> agenda-rows;
    callback media-action(string);

    VerticalLayout {
        padding: 14px;
        spacing: 6px;

        Text { text: root.time-text; color: root.accent; font-size: 20px; font-weight: 700; }
        Text {
            text: root.has-media ? root.media-title + " · " + root.media-artist : "Nothing playing";
            color: white;
        }
        Rectangle {
            height: 4px;
            background: #ffffff30;
            Rectangle { width: parent.width * root.media-progress; background: root.accent; }
        }
        for item in root.agenda-rows: Text {
            text: item.time + "  " + item.title;
            color: #ffffffb0;
        }
        Rectangle {
            height: 28px;
            background: root.accent;
            border-radius: 14px;
            Text { text: "⏭"; color: black; }
            TouchArea { clicked => { root.media-action("next"); } }
        }
    }
}
```

## Limits

- The view replaces **all** of the open content: it is up to you to show what
  you want to keep (time, music, agenda…). The compact pill does not change.
- Your view's animations are your responsibility: as for the island, avoid
  looping animations if you want to keep ~0% CPU at rest.
