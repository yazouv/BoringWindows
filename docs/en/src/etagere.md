# Shelf

Drop files onto the island to keep them at hand, like a little desk next to your
work: for the length of an email, an assignment upload or a transfer.

## Using it

1. Drag one or more files (from Explorer, a browser…) onto the pill: it opens by
   itself while you hover.
2. Release: a **Shelf** row appears with the file name.
3. **Click** a file: it opens with its usual application. **×** removes it from
   the shelf (the file itself is never touched).

Only the **path** is kept, not a copy: if the file is moved or deleted, it drops
off the shelf. The list survives restarts (`shelf.txt` next to `config.toml`).

The island shows the 4 latest files ("+3" counts the others). The oldest ones
leave when the limit is reached.

## Settings

Settings › **Shelf**, or in `config.toml`:

```toml
[modules.shelf]
enabled = true   # false: the island ignores drops
max = 8          # 1 to 30 files kept
```

Dragging files *out of* the island into another application is not supported:
click the file to open it, then use the application you want.
