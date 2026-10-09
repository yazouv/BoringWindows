# Themes

A theme sets the island's colors, border, font and corner rounding in one go.
Four are built in, plus an automatic one:

| Name | Look |
|---|---|
| `auto` | follows Windows: `light` in light mode, `default` in dark mode |
| `default` | black, orange accent (the classic island) |
| `light` | light, blue accent, thin border: for light backgrounds |
| `midnight` | midnight blue, lavender accent |
| `glass` | smoked glass: slightly see-through, light border, blurred background |

![The default, light, midnight and glass themes](images/themes.png)

## Picking a theme

**Settings… › Appearance › Theme.** The island changes right away. Picking a
theme replaces the colors you had customized with the theme's.

Or in `config.toml`:

```toml
[theme]
name = "midnight"
```

## Tweaking a theme

Keys written in `[theme]` take **precedence** over the theme's. To keep
"midnight" with a green accent:

```toml
[theme]
name = "midnight"
accent = "#30D158"
```

Island sizes, animation duration and the offset from the top of the screen are
not part of themes: they are always set in `[theme]` (see the
[reference](configuration.md#theme)).

## Windows accent color and blur

- **Settings… › Appearance › "Use the Windows accent color"**
  (`system_accent = true`): the accent follows the one chosen in Windows
  settings, in a lighter shade on dark themes and a darker one on light
  themes. It changes along with Windows. Album art still takes over while
  music plays (Music tab).
- **"Blur what is behind the island"** (`blur = true`, on in `glass`): what is
  behind the island is blurred, like Windows 11 menus. The blur takes the
  exact shape of the pill, animations included. It only shows through a
  translucent background (`background` with opacity below `FF`). It needs
  Windows 11 with transparency effects turned on (Settings › Personalization ›
  Colors).

```toml
[theme]
name = "auto"
system_accent = true
blur = true
background = "#1C1C1EB8"
```

## Making your own theme

1. **Settings… › Appearance › Themes folder** (the `themes` folder next to
   `config.toml` is created if needed).
2. Create a text file `my-theme.toml` (the file name is the theme name):

   ```toml
   background = "#2B1B3DF0"   # background, slightly transparent
   foreground = "#F5EFFF"     # text
   accent = "#FF7AC6"         # urgent, buttons, upcoming meeting
   border = "#FFFFFF26"       # border (transparent: none)
   font = "Segoe UI Variable" # a font installed on Windows (empty: system font)
   corner_radius = 24.0       # corners of the open island
   ```

   Every key is optional: what's missing comes from the `default` theme.
3. It shows up in the theme list (reopen the settings window), or put
   `name = "my-theme"` in `[theme]`.

The file is watched: save it and the island updates. A mistake (unknown key,
badly written color) is shown in the island, as for `config.toml`.

Colors are written `"#RRGGBB"` or `"#RRGGBBAA"` (the last two digits: opacity,
from `00` transparent to `FF` opaque). Secondary text, buttons and bars are
derived from `foreground` and `accent`.

To share a theme, just send its `.toml` file.
