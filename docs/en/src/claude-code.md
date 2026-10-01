# Claude Code

The island follows your [Claude Code](https://claude.com/claude-code)
sessions: it shows what Claude is doing, tells you when it's waiting for you,
and lets you **allow or deny a command without going back to the terminal**.

![Pill: question, permission request, then Claude resuming](images/claude-pilule.png)

## Connect Claude Code

1. Start BoringWindows.
2. Right-click its icon › **Claude Code: install hooks…**, or **Settings…** ›
   **Claude Code** tab › **Install hooks**.
3. The dialog lists exactly what will be added to `~/.claude/settings.json`.
   Answer **Yes**.
4. **Restart your Claude Code sessions**: they only read their hooks at start
   up.

What happens:

- a **dated backup** of `settings.json` is made before any change;
- only BoringWindows' entries are added: your other settings and your own
  hooks are left untouched;
- the relay is copied to `%LOCALAPPDATA%\BoringWindows\bin\bw-hook.exe`; it is
  refreshed automatically when BoringWindows is updated;
- the same menu ("remove hooks…") removes everything cleanly.

## What the island shows

| Pill | Meaning |
|---|---|
| ● grey · `project · Bash` | Claude is working (tool running) |
| ● grey · `project · thinking…` | Claude is writing its answer |
| ● orange · `project · allow Bash?` | **permission request**, to handle in the island |
| ● orange · `project · is asking you a question` | a question or a plan to approve: **answer in the terminal** |
| ● orange · `project · waiting for you` | Claude is done and waits for your next message |
| `project · done` | end of turn (a few seconds) |

With several sessions, the most urgent one takes the pill (`(+2)` counts the
others) and the open island lists them all. **Click a session** to bring its
terminal (Windows Terminal, VS Code…) to the front.

A **system sound** plays every time Claude starts waiting for you.

## Answering a permission request

When Claude wants to run a command that needs your approval, hover the island:
it shows the tool and the command, with three buttons.

- **Allow**: Claude runs the command.
- **Deny**: Claude doesn't run it and carries on another way.
- **Terminal**: the question goes back to the terminal, which comes to the
  front (to pick "always allow", for example).

Without an answer after 60 seconds, the question also goes back to the
terminal. If you answer directly in the terminal, the request disappears from
the island.

Claude's **questions** (multiple choice, approving a plan) don't use these
buttons: they show up normally in the terminal, the island just tells you
"is asking you a question".

## Settings

In [`[modules.claude]`](configuration.md#modulesclaude):

```toml
[modules.claude]
permissions = true          # answer permissions from the island
permission_wait_secs = 60   # delay before handing back to the terminal
sound = true                # sound when Claude is waiting for you
```

## If nothing shows up

Run the diagnostic, with BoringWindows running, in another terminal:

```powershell
boringwindows.exe doctor
```

It checks the whole chain and tells you where it breaks. Details in
[Troubleshooting](depannage.md#claude-code-nothing-shows-up).

## Privacy

The relay only sends the island a **summary**: project name, tool, command or
file name. Never the content of your files nor your messages to Claude. The
channel (a *named pipe*) is local and restricted to your Windows account. If
BoringWindows is closed or doesn't answer within 300 ms, the relay steps aside
and does nothing: **Claude is never blocked**.

## Recent conversations and usage

Open the island and click **Claude ›** (top right): a tab shows

- an **estimate of your usage** over the current 5-hour window, with the time
  until it renews (and a gauge if you set a limit);
- your **4 latest conversations** (title, folder, age). Click one to reopen it: a
  terminal opens in its folder and runs `claude --resume <id>`.

It is read from the transcripts Claude Code keeps locally
(`~/.claude/projects`): no connection, nothing is sent. Transcripts are only
re-read when the island opens (at most every 15 s), and only the files that
changed.

**Usage is an estimate, not your real quota.** Your subscription's limits live
server-side and Claude Code does not publish them. BoringWindows adds up the
tokens (input, output, cache creation) of the messages in the current window, as
the `ccusage` tool does; for a gauge, enter the limit you believe you have
(millions of tokens) in Settings › Claude Code. For your real remainder, `/usage`
in Claude Code is the reference.

**Remote** conversations (SSH sessions) are not listed: their folder does not
exist on this PC. They still count toward usage.

```toml
[modules.claude_activity]
enabled = true
recent = 4                # conversations offered (0 to 4)
window_hours = 5          # usage window length (1 to 24)
limit_tokens = 0          # estimated limit for the gauge (0: no gauge)
reset_at = ""             # end of a known window (see below)
count_cache_reads = false # also count cache reads
```

Reopening uses Windows Terminal (`wt`) if installed, otherwise a console, and
assumes the `claude` command is on your `PATH`. Windows only for now.

### Aligning the countdown

Your plan's 5-hour window is **shared across the whole account**: it also counts
claude.ai and the Claude app, which BoringWindows cannot see. As a result,
without alignment the window start is inferred from Claude Code messages alone
and the "resets in…" can be off (by several hours).

To align it, type the reset time Claude shows (`/usage`, or claude.ai) in
Settings › Claude Code › **Window reset time**, as `HH:MM` (local time, e.g.
`03:01`). BoringWindows keeps that window end; once it has passed, it starts over
from the following messages, with the exact time of the first message as the start.
So the field is only needed after activity outside Claude Code.
