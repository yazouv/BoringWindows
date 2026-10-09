# Weather

The current weather shows up next to the date in the open island: an icon
(sun, clouds, rain, snow, storm…) and the temperature. **Hover it**: the date
makes room for the day's details, for example "Lyon · Partly cloudy · 9° /
16°".

Off by default: it needs a city.

Settings › **Weather**: tick "Show the weather next to the date in the open
island", type your city and pick the unit. Or in `config.toml`:

```toml
[modules.weather]
enabled = true
city = "Lyon"            # or "Lyon, France" to remove ambiguity
units = "celsius"        # or "fahrenheit"
refresh_minutes = 30     # 10 to 180
```

For an exact place, give its coordinates instead of the city (they take
precedence over `city`):

```toml
[modules.weather]
enabled = true
latitude = 45.76
longitude = 4.84
```

## Where the data comes from

From [open-meteo.com](https://open-meteo.com): free, no account or key. Only
the city name (once, to find its coordinates) and the coordinates go over the
Internet. If the request fails (no network after waking up…), it is retried
two minutes later; the log says more (`météo : …`).
