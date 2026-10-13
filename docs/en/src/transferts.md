# File transfers

When you **copy** files in File Explorer or **download** something with your
browser, a **progress bar** shows at the bottom edge of the collapsed pill. It
sits on top of whatever the pill already shows (music, for example): you don't
have to open anything to see how far along it is.

## Copies, moves, deletions

File Explorer opens a progress window for longer operations. The island follows
it: the bar advances with the percentage, and the pill shows for example "42 % ·
Copying 3 items from Documents to Backup" when nothing more important owns the
pill.

It all comes from Windows events (the window appearing and its title
changing): nothing is polled. Windows only.

## Downloads

Browsers write a download into a temporary file (`.crdownload` for Chrome,
Edge and Opera, `.part` for Firefox…), then rename it when it is done. The
island watches your Downloads folder and follows those files.

Browsers don't tell the **final size**: the bar moves back and forth instead
of filling up, and the pill shows the size received and the speed, for example
"video.mp4 · 128 MB · 3.2 MB/s". With several downloads, it adds them up
("2 downloads · 340 MB · 5.1 MB/s").

The folder is only sampled (once per second) **while a temporary file
exists**; the rest of the time, the island just waits for a change in the
folder.

A temporary file that has not been written to for more than **30 seconds**
(abandoned or paused download, a `.part` left behind by Firefox) is not
counted: the island doesn't get stuck on an old file. It is picked up again as
soon as it grows.

A cancelled download disappears without any announcement.

## Announcements

When a transfer finishes, the pill shows "Downloaded: video.mp4" or "File
operation finished" for a few seconds. A cancelled copy is not announced.

## Settings

Settings › **System** › *Transfers*, or in `config.toml`:

```toml
[modules.transfers]
enabled = true
copies = true              # File Explorer copies, moves and deletions
downloads = true           # browser downloads
downloads_folder = ""      # empty: the system Downloads folder
announce_done = true       # announce when a transfer finishes
show_secs = 5              # how long the announcement stays, 2 to 30
```

If your downloads go somewhere other than the Windows Downloads folder, set
`downloads_folder`.
