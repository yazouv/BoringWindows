# Toutes les options (config.toml)

Le fichier se trouve dans `%APPDATA%\BoringWindows\config.toml` (clic droit
sur l'icône › **Ouvrir la configuration**). Il est créé au premier lancement
avec toutes les options commentées. Les réglages courants se changent aussi
sans toucher au fichier : clic droit › **Réglages…**.

- **Enregistre : c'est appliqué aussitôt**, sans redémarrage.
- Une option absente prend sa valeur par défaut : tu peux ne garder que ce que
  tu changes.
- Une faute de frappe (option inconnue, valeur hors limites) est signalée dans
  l'île ; les réglages précédents restent en place.

## Rappel de syntaxe

```toml
# Un commentaire
[section]                 # une section
option = "texte"          # texte entre guillemets
nombre = 42
interrupteur = true       # true ou false
liste = ["a", "b"]

[[modules.calendar.sources]]   # double crochet : un élément de liste, à répéter
name = "Pro"
```

> Chemins Windows : écris-les avec des `/` (`"C:/Users/moi/a.ics"`) ou entre
> guillemets simples (`'C:\Users\moi\a.ics'`). Avec des guillemets doubles,
> les `\` sont interprétés et le fichier devient invalide.

## `[general]`

| Option | Défaut | Valeurs | Rôle |
|---|---|---|---|
| `monitor` | `"primary"` | `"primary"`, `"cursor"` | écran de l'île : principal, ou celui où se trouve la souris au lancement |
| `hide_in_fullscreen` | `true` | `true`, `false` | cacher l'île quand une application est en plein écran |
| `auto_update` | `true` | `true`, `false` | installer les nouvelles versions automatiquement (voir [Installation](installation.md#mises-à-jour)) |
| `language` | `"auto"` | `"auto"`, `"fr"`, `"en"` | langue de l'interface (`auto` : celle de Windows) |
| `open_on` | `"hover"` | `"hover"`, `"click"` | ouvrir l'île au survol ou au clic |
| `collapse_delay_ms` | `350` | 0 à 10000 | délai avant de refermer quand la souris s'en va (ms) |

## `[theme]`

| Option | Défaut | Valeurs | Rôle |
|---|---|---|---|
| `name` | `"default"` | `"default"`, `"light"`, `"midnight"`, `"glass"` ou un thème perso | thème de base (voir [Thèmes](themes.md)) ; les clés ci-dessous passent devant |
| `background` | selon le thème | `"#RRGGBB"` ou `"#RRGGBBAA"` | fond de l'île |
| `foreground` | selon le thème | idem | texte |
| `accent` | selon le thème | idem | couleur d'accent (urgent, boutons) ; remplacée par la couleur de la pochette quand la musique joue (voir `[modules.media]`) |
| `border` | selon le thème | idem | contour de l'île (`"#00000000"` : aucun) |
| `font` | `""` | nom d'une police installée | police de l'île (vide : celle du système) |
| `corner_radius` | selon le thème | 0 à 500 | arrondi des coins de l'île ouverte |
| `animation_ms` | `240` | 0 à 2000 | durée des animations, `0` pour aucune |
| `top_offset` | `0.0` | 0 à 500 | décalage depuis le haut de l'écran ; au-delà de 0, les coins du haut s'arrondissent aussi |

Tailles (en pixels, avant mise à l'échelle de Windows) :

| Section | Défaut | Rôle |
|---|---|---|
| `[theme.compact]` | `width = 190.0`, `height = 32.0` | pilule au repos |
| `[theme.attention]` | `width = 300.0`, `height = 36.0` | pilule quand un module a quelque chose à dire |
| `[theme.expanded]` | `width = 520.0`, `height = 170.0` | île ouverte ; doit être au moins aussi grande que les deux autres |

Largeur entre 16 et 4000, hauteur entre 8 et 2000.

## `[layout]`

| Option | Défaut | Rôle |
|---|---|---|
| `compact` | `["claude", "media", "calendar"]` | ordre de priorité des modules **à importance égale** (une urgence passe toujours devant) |
| `view` | `""` | nom d'un fichier de `layouts/` (sans `.slint`) : [vue personnelle](layouts.md) de l'île ouverte |

## `[modules.claude]`

Voir [Claude Code](claude-code.md).

| Option | Défaut | Valeurs | Rôle |
|---|---|---|---|
| `enabled` | `true` | | activer le module |
| `permissions` | `true` | | répondre aux demandes de permission depuis l'île |
| `permission_wait_secs` | `60` | 5 à 280 | délai pour répondre dans l'île avant de rendre la main au terminal (s) |
| `done_secs` | `8` | 0 à 600 | durée d'affichage de « terminé » (s) |
| `sound` | `true` | | son système quand Claude se met à t'attendre |

## `[modules.media]`

Voir [Musique](musique.md).

| Option | Défaut | Rôle |
|---|---|---|
| `enabled` | `true` | activer le module |
| `accent_from_artwork` | `true` | teinter l'île avec la couleur dominante de la pochette |
| `ignore` | `[]` | lecteurs à ignorer, par morceau de nom : `["msedge", "chrome"]` |

## `[modules.calendar]`

Voir [Agenda](agenda/index.md).

| Option | Défaut | Valeurs | Rôle |
|---|---|---|---|
| `enabled` | `true` | | activer le module |
| `remind_minutes` | `5` | 0 à 120 | rappel avant le début d'un événement (min) |
| `refresh_minutes` | `10` | 2 à 1440 | fréquence de téléchargement des calendriers (min) |
| `lookahead_hours` | `24` | 1 à 168 | horizon affiché dans l'île (h) |
| `show_all_day` | `true` | | afficher les événements « toute la journée » |

Chaque calendrier est un bloc `[[modules.calendar.sources]]` :

| Option | Obligatoire | Rôle |
|---|---|---|
| `name` | non | nom affiché dans les messages |
| `url` | oui | lien `https://` ou `webcal://`, chemin d'un fichier `.ics`, ou `secret:<id>` (valeur gardée dans le Gestionnaire d'identifiants) ; pour CalDAV, adresse du serveur |
| `kind` | non | `"ics"` (défaut) ou `"caldav"` — voir [CalDAV](agenda/caldav.md) |
| `username` | CalDAV | identifiant de connexion |
| `password` | CalDAV | mot de passe d'application, de préférence `secret:<id>` |

## `[modules.timer]`

| Option | Défaut | Plage | Rôle |
|---|---|---|---|
| `enabled` | `false` | | activer le [minuteur](minuteur.md) |
| `presets` | `[5, 15, 25]` | 1 à 5 durées, 1 à 600 | durées proposées (min) |
| `sound` | `true` | | son à la fin |
| `done_secs` | `20` | 1 à 600 | durée de l'alerte « terminé » (s) |

## `[modules.shelf]`

| Option | Défaut | Plage | Rôle |
|---|---|---|---|
| `enabled` | `true` | | garder les fichiers déposés sur l'[étagère](etagere.md) |
| `max` | `8` | 1 à 30 | nombre de fichiers gardés |

## `[modules.volume]`

| Option | Défaut | Plage | Rôle |
|---|---|---|---|
| `enabled` | `false` | | afficher les changements de [volume](volume.md) dans l'île (Windows) |
| `show_secs` | `2` | 1 à 10 | durée d'affichage (s) |

## `[modules.visualizer]`

| Option | Défaut | Plage | Rôle |
|---|---|---|---|
| `enabled` | `false` | | [visualiseur audio](musique.md#visualiseur-audio-optionnel) (Windows) |
| `bands` | `12` | 4 à 32 | nombre de barres |
| `fps` | `30` | 10 à 60 | images par seconde |

## `[modules.demo]`

| Option | Défaut | Rôle |
|---|---|---|
| `enabled` | `false` | module de démonstration : faux lecteur et alertes qui défilent, pour essayer l'île sans rien configurer |

## Exemple complet

```toml
[general]
open_on = "hover"

[theme]
accent = "#5AC8FA"

[modules.calendar]
remind_minutes = 10
lookahead_hours = 48

[[modules.calendar.sources]]
name = "Pro"
url = "https://outlook.office365.com/owa/calendar/…/calendar.ics"

[[modules.calendar.sources]]
name = "Cours"
url = "https://edt.exemple.fr/calendar/G7a.ics"

[modules.media]
ignore = ["msedge"]
```
