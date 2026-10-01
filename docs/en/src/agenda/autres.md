# Other calendars and .ics files

## School timetable, intranet, shared schedule

Many university timetables (ADE, HyperPlanning, Celcat…), school intranets,
clubs or booking tools offer an **iCal export** or **subscription** link that
updates itself. Look for "iCal", "ICS", "Export", "Subscribe" or "Sync" on the
timetable page. The link often ends with `.ics`, or starts with `webcal://`.

```toml
[[modules.calendar.sources]]
name = "Timetable"
url = "https://timetable.example.edu/api/v1/calendar/G7a.ics"
```

Each class's room is shown next to the time: `Dans 3 min · 112 · R5A.07 -
Automatisation…`.

To also see the next day from the morning, widen the horizon:

```toml
[modules.calendar]
lookahead_hours = 48
```

## .ics file on your disk

An exported file, or one received by email, works too. Easiest: **Settings…**
› **Calendar** › service ".ics file on disk" › **Browse…**, then **Test** and
**Add**.

By hand, give its path:

```toml
[[modules.calendar.sources]]
name = "Schedule"
url = "C:/Users/me/Documents/schedule.ics"
```

Write the path with `/`, as above. If you paste a path with `\`, put it in
**single quotes** (otherwise the configuration file is invalid):

```toml
url = 'C:\Users\me\Documents\schedule.ics'
```

The file is read again every 10 minutes: if you replace it with a new version,
it is picked up.

## Compatibility

Any calendar in standard iCalendar format works: Fastmail, Nextcloud,
Thunderbird, Zimbra, Zoho… If one of them displays badly, open an
[issue](https://github.com/yazouv/BoringWindows/issues) with an excerpt of the
file (with personal information removed).
