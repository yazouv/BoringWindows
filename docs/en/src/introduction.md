# BoringWindows

**A "Dynamic Island" for Windows**, at the top of your screen: unobtrusive when
nothing is happening, it comes alive when something deserves your attention and
opens when you hover it.

![The island: compact, then open with music and a Claude session](images/musique.png)

What it shows:

- 🤖 **[Claude Code](claude-code.md)**: your sessions live (working, asking you
  a question, done) and permission requests, which you can answer without
  leaving what you are doing.
- 🎵 **[Music](musique.md)**: what's playing (Spotify, Apple Music, browser,
  VLC…), artwork, previous / pause / next. Nothing to set up.
- 📅 **[Calendar](agenda/index.md)**: your next classes or meetings (Google,
  Outlook, iCloud, school timetables…), a reminder before they start and a
  **Join** button for Teams, Meet or Zoom.
- 🔋 **[Battery and Bluetooth](energie.md)**: charger plugged in, low battery,
  headphones connected with their battery level.
- 🌤️ **[Weather](meteo.md)**: the current weather, next to the date.

It is lightweight (written in Rust, no embedded browser), never steals focus
and hides itself when a video or a game goes full screen.

> The app follows the language of Windows (English or French); you can change
> it in **Settings… › General › Language**.

👉 Start with the [installation](installation.md).
