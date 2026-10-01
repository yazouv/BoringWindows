# Google Calendar

Google gives every calendar a **secret address in iCal format**.

1. Open [Google Calendar](https://calendar.google.com) in a browser (not the
   mobile app).
2. Top right: ⚙️ **Settings**.
3. In the left column, under **Settings for my calendars**, click the
   calendar.
4. Scroll down to **Integrate calendar**.
5. Copy **Secret address in iCal format** (it ends with `basic.ics`).

> Don't take the "Public address in iCal format": it only works if the
> calendar is made public.

Then in `config.toml`:

```toml
[[modules.calendar.sources]]
name = "Google"
url = "https://calendar.google.com/calendar/ical/…/private-…/basic.ics"
```

Repeat for each calendar (personal, family…): one block per calendar.

**Notes**

- Google sometimes takes several hours to reflect a change in this link. For
  last-minute changes, the Google app remains faster.
- Google Meet links in invitations are detected: **Join** button.
- **Link compromised?** Same page, **Reset** button under the secret address:
  the old link stops working.
- With a Google Workspace account (work, school), the administrator may have
  turned this option off.
