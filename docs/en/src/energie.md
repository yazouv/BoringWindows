# Battery and Bluetooth

## PC battery

On a laptop, the island briefly shows what happens to the battery, with a bolt
and a gauge:

- **charger plugged in**: "Charging · 54 %", green bolt and gauge;
- **charger unplugged**: "On battery · 54 %";
- **fully charged** (100 %, charger plugged in);
- **low battery**: when it drops below the threshold (20 % by default), then
  at half the threshold (10 %), in red.

On by default. On a desktop PC, with no battery, the module does nothing.

## Bluetooth devices

When Bluetooth headphones, earbuds, a mouse or a keyboard connects, the island
shows its name and battery level: "WH-1000XM5 · 80 %". The level often arrives
a few seconds after the connection: the island then shows it in turn.
Disconnections and low battery (below the threshold) show up too.

- The level comes from Windows: it is the one shown in Settings › Bluetooth &
  devices. A device that doesn't report it to Windows shows up without a
  gauge.
- Headphones and earbuds get a headphones icon, other devices the Bluetooth
  icon.

On by default.

## Settings

Settings › **Notifications** › "Battery" and "Bluetooth". Or in
`config.toml`:

```toml
[modules.battery]
enabled = true
show_secs = 4     # how long it shows (1 to 10 s)
low_percent = 20  # low battery threshold (5 to 50 %)

[modules.bluetooth]
enabled = true
show_secs = 4
low_percent = 20
```

Everything goes through Windows events (power management, device watchers):
nothing is polled, nothing runs while idle. Windows only.
