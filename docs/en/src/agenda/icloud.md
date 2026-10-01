# iCloud

iCloud publishes a calendar as a **public calendar**: a hard-to-guess
`webcal://` link, which BoringWindows accepts as is.

**From an iPhone or iPad**

1. Open the **Calendar** app › **Calendars** (at the bottom).
2. Tap ⓘ next to the calendar.
3. Turn on **Public Calendar**, then **Share Link…** › **Copy**.

**From a Mac**: Calendar app, right-click the calendar › **Share Calendar…** ›
tick **Public Calendar** › copy the address.

**From [iCloud.com](https://www.icloud.com/calendar)**: share icon next to the
calendar › **Public Calendar** › **Copy Link**.

Then in `config.toml`:

```toml
[[modules.calendar.sources]]
name = "iCloud"
url = "webcal://p12-caldav.icloud.com/published/2/…"
```

**Notes**

- "Public" means: anyone with the link can read the calendar. Keep it to
  yourself.
- To revoke access: turn off **Public Calendar** (then turn it on again to get
  a new link).
