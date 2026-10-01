# Claude Code

The island follows your [Claude Code](https://claude.com/claude-code)
sessions: it shows what Claude is doing, tells you when it's waiting for you,
and lets you **allow or deny a command without going back to the terminal**.

![Pill: question, permission request, then Claude resuming](images/claude-pilule.png)

## Connect Claude Code

1. Start BoringWindows.
2. Right-click its icon › **Claude Code : installer les hooks…** (install
   hooks), or **Réglages…** › **Claude Code** tab › **Installer les hooks**.
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
- the same menu ("retirer les hooks…") removes everything cleanly.

## What the island shows

The texts are in French for now:

| Pill | Meaning |
|---|---|
| ● grey · `project · Bash` | Claude is working (tool running) |
| ● grey · `project · réfléchit…` | Claude is writing its answer |
| ● orange · `project · autoriser Bash ?` | **permission request**, to handle in the island |
| ● orange · `project · te pose une question` | a question or a plan to approve: **answer in the terminal** |
| ● orange · `project · attend ta réponse` | Claude is done and waits for your next message |
| `project · terminé` | end of turn (a few seconds) |

With several sessions, the most urgent one takes the pill (`(+2)` counts the
others) and the open island lists them all. **Click a session** to bring its
terminal (Windows Terminal, VS Code…) to the front.

A **system sound** plays every time Claude starts waiting for you.

## Answering a permission request

When Claude wants to run a command that needs your approval, hover the island:
it shows the tool and the command, with three buttons.

- **Autoriser** (allow): Claude runs the command.
- **Refuser** (deny): Claude doesn't run it and carries on another way.
- **Terminal**: the question goes back to the terminal, which comes to the
  front (to pick "always allow", for example).

Without an answer after 60 seconds, the question also goes back to the
terminal. If you answer directly in the terminal, the request disappears from
the island.

Claude's **questions** (multiple choice, approving a plan) don't use these
buttons: they show up normally in the terminal, the island just tells you
"te pose une question".

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
