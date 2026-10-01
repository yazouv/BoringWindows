# CalDAV: iCloud, Fastmail, Nextcloud…

CalDAV is the calendar sync protocol. With it, BoringWindows reads **every
calendar of your account** (no need to publish each calendar or copy an ICS
link). In exchange you need a login and an **app password**: never your main
password.

The password is kept in the **Windows Credential Manager**, not in
`config.toml`, which only holds `password = "secret:…"`.

## What to enter

| Service | Server address | Login | Password |
|---|---|---|---|
| iCloud | `https://caldav.icloud.com` | your Apple ID | [app-specific password](https://account.apple.com/account/manage): Sign-In and Security › App-Specific Passwords |
| Fastmail | `https://caldav.fastmail.com` | your Fastmail address | Settings › Privacy & Security › App passwords (access to "Calendars") |
| Nextcloud | `https://your-server/remote.php/dav` | your username | Settings › Security › Create new app password |

The address of one specific calendar works too: BoringWindows then reads only
that one.

## Add the account

Settings › **Calendar** › **CalDAV** › address, login, app password › **Test**
› **Add**.

By hand, in `config.toml`:

```toml
[[modules.calendar.sources]]
name = "iCloud"
kind = "caldav"
url = "https://caldav.icloud.com"
username = "me@icloud.com"
password = "secret:caldav-18f3a2"   # kept in the Credential Manager
```

A plain-text `password` works too (useful on Linux, which has no vault), but
anyone who opens the file can read it.

## Notes

- Private ICS links (Google, Outlook…) added from the settings window are
  stored the same way: `url = "secret:ics-…"`. Removing the calendar from the
  settings also deletes the secret.
- If **Test** fails: check the address (without `/principal`), the login, and
  that the password really is an *app* password.
