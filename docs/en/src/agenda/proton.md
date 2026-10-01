# Proton Calendar

Proton can share a calendar with a read-only **link**.

1. Open [Proton Calendar](https://calendar.proton.me) on the web.
2. ⚙️ **Settings** › **Calendars** › click the calendar.
3. Under **Share with anyone**: **Create link**.
4. Choose **See all event details**, then copy the link.

Then in `config.toml`:

```toml
[[modules.calendar.sources]]
name = "Proton"
url = "https://calendar.proton.me/api/calendar/v1/url/…/calendar.ics?…"
```

**Notes**

- This link contains the decryption key of your calendar: treat it like a
  password.
- To revoke access: same page, delete the link.
