# Quick tasks

A list of **checkboxes** in the island, to jot down what needs doing without
leaving what you are doing. Off by default: Settings › **System** › *Tasks*,
or `enabled = true` in `[modules.tasks]`.

## The Tasks tab

A **Tasks** tab is added to the open island, with the number of tasks to do as
a badge. In the tab:

- **click a task** to check it off (it turns gray, at the end of the list),
  click it again to uncheck it;
- **×** (on hover) removes it;
- **+ Add a task** opens the add box;
- **Clear done** removes all checked tasks;
- when there are more than fit on screen, "+3" says how many are hidden.

## Adding a task

The island never takes the keyboard (that is what lets it not steal the focus
from what you are typing): you can't type in it. Instead, the **Ctrl+Alt+N**
shortcut opens a small box, **under the island**, that receives the keyboard:

- **Enter** adds the task and closes the box;
- **Esc** cancels.

Change the shortcut in Settings › *Tasks* › *Add a task*, or with `quick_add`
in `[hotkeys]`. It is only active when tasks are enabled.

## The file

Tasks live in a **Markdown** file, `tasks.md`, next to `config.toml` (Settings
› *Tasks* › *Open*):

```markdown
# Shopping

- [ ] Buy milk
- [x] Call Marie
A free note.
- [ ] Pay the bill
```

Only `- [ ] …` and `- [x] …` lines (or with `*` and `+`) are tasks.
**Everything else is kept as is**: headings, notes, blank lines. So you can
edit it with any editor, keep it in Obsidian, or sync it with a file service.

The island rereads the file every time it opens, and before every change: a
task added by hand shows up without restarting, and nothing is overwritten. A
task added from the island goes after the others. Tasks are limited to 200
characters and fit on one line.

## Settings

```toml
[modules.tasks]
enabled = false      # off by default
file = ""            # empty: tasks.md next to config.toml

[hotkeys]
quick_add = "Ctrl+Alt+N"
```
