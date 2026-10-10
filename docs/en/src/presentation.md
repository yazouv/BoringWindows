# Presentation mode

During a call or screen sharing, the island keeps quiet:

- **no notification pops up** (they still land in the "Notifs" tab);
- a **red dot** shows in the pill, to remind you that you are live.

And, in a call or not, **the island never appears in screen sharing,
recordings and screenshots**: your notifications don't leak during a demo.
Note that this also applies to your own screenshots (Win + Shift + S): untick
the option if you want to capture the island.

## How it is detected

Windows has no "screen is shared" API, but it records, app by app, who uses
the microphone and who captures the screen (this is what drives the
microphone icon in the taskbar). The island considers you live when:

- an app **captures the screen** (Teams, Discord, Zoom, OBS… with the Windows
  capture);
- or a **call app** uses the microphone (`call_apps` list, by default Teams,
  Discord, Zoom, Slack, Webex, Skype, browsers for Meet, OBS).

Detection reacts to changes (nothing is polled). An app that crashes
sometimes leaves its entry "in progress": the island checks that the app is
still running.

## Settings

Settings › **Notifications** › "Presentation mode" and "Hide from
capture", or in `config.toml`:

```toml
[modules.presentation]
enabled = true            # detect calls and screen sharing
hide_from_capture = true  # the island doesn't appear in captures
call_apps = ["teams", "discord", "zoom", "slack", "webex", "skype", "chrome", "msedge", "firefox", "brave", "opera", "obs"]
```

Windows only.
