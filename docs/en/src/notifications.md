# Notifications

When an app sends you a Windows notification (a Discord, Slack, Teams or
WhatsApp message, an Outlook email…), the island announces it: the pill widens,
a badge with the app's initial pops in with a ripple in its color, then the
sender and the message. After a few seconds, the pill goes back to its size.

Until you look at them, notifications leave **one colored dot** per app on the
right of the pill.

## The "Notifs" tab

Open the island and click the **Notifs** tab (top right), or open the island
while a notification is showing: it opens straight on that tab.

- The 4 latest notifications, with their age.
- **Click** a row to open (or bring back) the app that sent it.
- The **×** that shows on hover clears the notification, from the Windows
  notification center too. **Clear all** clears the ones in the list.
- The tab's badge counts the notifications that arrived since your last visit.

## Do not disturb

Click the **moon** at the top right of the open island. Notifications keep
arriving in the **Notifs** tab, but no longer pop up in the pill or leave dots:
a small moon shows instead. Click the moon again to go back to normal; the dots
for what you missed come back.

The mode is kept across restarts (a `do-not-disturb` file next to
`config.toml`). It doesn't follow Windows' own "Do not disturb".

## Settings

On by default. Settings › **General** › "Show app notifications in the island",
or in `config.toml`:

```toml
[modules.notifications]
enabled = true
show_secs = 5          # how long a new notification shows (1 to 30)
show_content = true    # false: only the app name, without the message
ignore = ["docker"]    # apps to ignore (part of a name)
```

`show_content = false` helps when you share your screen: the island announces
"Discord · New notification" without showing who wrote or what.

## Good to know

- BoringWindows reads the Windows **notification center**. If the island shows
  nothing, check Windows Settings › Privacy & security › **Notifications**:
  notification access must be allowed for desktop apps.
- An app whose notifications you turned off in Windows doesn't show in the
  island either.
- Windows doesn't tell unpackaged apps when a notification arrives, so the
  island re-reads the list every 2 seconds. It's a light call, but it's the only
  exception to BoringWindows' "no polling" rule. Turn the module off to avoid
  it.
- The color comes from the app (Discord blue, WhatsApp green…). An unknown app
  gets a stable color derived from its name.
- A notification takes over the pill from anything, except a request waiting
  for your answer (a Claude Code permission).
- Windows only.
