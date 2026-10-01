# Outlook and Microsoft 365

Outlook can **publish a calendar** as an ICS link. The steps are the same for
Outlook.com (personal account) and Microsoft 365 (work or school account).

1. Open Outlook on the web:
   - personal account: [outlook.live.com](https://outlook.live.com/calendar)
   - work / school account: [outlook.office.com](https://outlook.office.com/calendar)
2. ⚙️ **Settings** › **Calendar** › **Shared calendars**.
3. Under **Publish a calendar**:
   - select the calendar;
   - select **Can view all details** (otherwise you only get "Busy", without
     title or meeting link);
   - click **Publish**.
4. Two links appear: copy the **ICS** link (not the HTML one).

Then in `config.toml`:

```toml
[[modules.calendar.sources]]
name = "Outlook"
url = "https://outlook.office365.com/owa/calendar/…/calendar.ics"
```

**Notes**

- Teams links in invitations are detected: **Join** button.
- Outlook-specific time zones ("Romance Standard Time"…) are handled.
- Outlook updates this link with a few minutes' delay.
- **No "Publish a calendar" section?** Your organisation has disabled it. A
  direct Microsoft 365 connection is planned for a future version.
- **Link compromised?** Same page, **Unpublish**, then publish again: a new
  link is created.
- The classic Outlook desktop app doesn't have this option: use Outlook on the
  web, once is enough.
