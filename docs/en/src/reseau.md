# Network

A small **icon** next to the time, in the open island, tells you where you
stand:

| Icon | Meaning |
|---|---|
| Wi-Fi | connected over Wi-Fi |
| Network port | connected over a cable |
| Shield | a VPN is connected |
| Crossed-out Wi-Fi (red) | no Internet connection |

**Hover the icon**: a bubble under the header gives the details, for example
"Home · ↓ 2.3 MB/s  ↑ 120 KB/s", or "Home · VPN · ↓ …" with a VPN.

The connection state comes from Windows events: nothing is polled.

## Speed in the collapsed pill

By default, the speed is only visible in the open island. You can also show it
**in the collapsed pill**, with `compact_speed`:

| Value | Effect |
|---|---|
| `off` | never (default) |
| `active` | only while data moves: a download, an upload… It disappears after a few quiet seconds |
| `always` | all the time |

The pill then shows "↓ 2.3 MB/s   ↑ 120 KB/s". It ranks **after** music and
Claude (order of `layout.compact`): while a track plays, it owns the pill. Put
`network` first in the list to reverse that:

```toml
[layout]
compact = ["network", "claude", "media", "calendar"]
```

The speed is only measured while it is visible: island open, or collapsed
pill set to `active` or `always`. With `off`, nothing is measured while the
island is closed.

With a VPN connected, bytes go through both the tunnel and the real adapter:
the island then keeps the busiest interface instead of the sum, so nothing is
counted twice.

## Announcements

The pill tells you when:

- the **Internet connection is lost** ("No internet connection") or
  **restored** ("Back online");
- a **VPN connects** or **disconnects**. A disconnection is flagged as
  urgent: your traffic is no longer protected;
- you **switch Wi-Fi network** ("Connected to Office"), if you enable it.

Each announcement is set separately.

> The VPN is detected from a **network adapter that is up**, with a usable IP
> address, whose type is "tunnel" (Windows VPN) or whose name is that of a VPN:
> OpenVPN (TAP, Wintun, DCO adapters), WireGuard, FortiClient, Cisco AnyConnect,
> GlobalProtect, Mullvad, NordVPN… A client that names its adapter differently
> will not be recognized.

## Settings

Settings › **System** › *Network*, or in `config.toml`:

```toml
[modules.network]
enabled = true
compact_speed = "off"        # off, active or always
unit = "bytes"               # bytes (MB/s) or bits (Mb/s)
refresh_secs = 1             # 1 to 10
active_kb_per_sec = 50       # active mode: display threshold (KB/s)
announce_connection = true   # connection lost and back
announce_vpn = true          # VPN connected or disconnected
announce_wifi = false        # Wi-Fi network change
show_secs = 5                # how long an announcement stays, 2 to 30
```

The connection state (Wi-Fi, VPN) is Windows only. The speed also works on
macOS and Linux.
