# Vues personnelles (.slint)

Tu peux remplacer le contenu de l'île ouverte par ta propre vue, écrite en
[Slint](https://slint.dev) : un simple fichier texte, rechargé à chaud dès que
tu l'enregistres, sans recompiler BoringWindows.

## Mise en place

1. Réglages › **Apparence** › **Dossier des vues** (ou crée
   `%APPDATA%\BoringWindows\layouts\` à la main).
2. Écris `layouts\ma-vue.slint` (exemple plus bas).
3. Réglages › **Apparence** › **Vue personnelle de l'île ouverte** › `ma-vue`, ou dans
   `config.toml` :

```toml
[layout]
view = "ma-vue"   # fichier layouts/ma-vue.slint ; vide = vue fournie
```

Une erreur dans le fichier s'affiche dans l'île (`⚠ ma-vue.slint:12 …`) et la
vue fournie reste en place jusqu'à ce que tu la corriges.

## Le fichier

Il exporte un composant `View` qui hérite de `Window`. Il remplit la zone de
l'île ouverte (taille réglée par `[theme.expanded]`). Les widgets standard
(`import { Button } from "std-widgets.slint";`, style Fluent) sont disponibles,
ainsi que les autres `.slint` du même dossier (`import "autre.slint";`).

Il suffit de **déclarer** les propriétés et callbacks dont tu as besoin : ceux
que tu ne déclares pas sont ignorés.

## Données (propriétés `in property`)

| Propriété | Type | Contenu |
|---|---|---|
| `expanded` | bool | l'île est ouverte |
| `time-text`, `date-text` | string | heure et date |
| `accent`, `foreground`, `background` | color | couleurs du thème (l'accent suit la pochette) |
| `has-media` | bool | une musique est détectée |
| `media-title`, `media-artist`, `media-source` | string | morceau en cours |
| `media-playing` | bool | en lecture |
| `media-position`, `media-duration` | string | « 1:23 », « 3:45 » |
| `media-progress` | float | 0 à 1 |
| `has-media-art`, `media-art` | bool, image | pochette |
| `media-multi-source`, `media-can-previous`, `media-can-next`, `media-can-toggle`, `media-can-seek` | bool | ce que le lecteur permet |
| `has-prompt`, `prompt-id`, `prompt-project`, `prompt-tool`, `prompt-detail` | bool, string | demande d'autorisation de Claude Code |
| `claude-rows` | `[{ id, project, status, kind: string, urgent, active: bool }]` | sessions Claude Code (`kind` : `wait`, `work`, `done` ou `idle`) |
| `agenda-rows` | `[{ title, time, location, relative, join-url: string, has-join, soon: bool }]` | prochains événements |
| `viz-bars` | `[float]` | niveaux 0 à 1 du visualiseur (vide si éteint) |
| `shelf-rows`, `shelf-more` | `[{ name, path: string }]`, string | fichiers de l'étagère (4 au plus), « +n » |
| `recent-rows` | `[{ id, title, meta, project, ago: string }]` | conversations Claude Code récentes (4 au plus) |
| `usage-text`, `usage-ratio`, `usage-has-limit` | string, float, bool | consommation estimée, part de la limite (0 à 1) |
| `plugin-rows` | `[{ name, text: string, attention: int }]` | lignes des plugins WASM (2 au plus) |
| `has-timer`, `timer-phase`, `timer-time`, `timer-progress`, `timer-presets` | bool, int, string, float, `[string]` | minuteur (phase : 0 prêt, 1 en cours, 2 pause, 3 terminé) |

## Actions (callbacks)

| Callback | Effet |
|---|---|
| `media-action(string)` | `"prev"`, `"toggle"`, `"next"`, `"source"` |
| `media-seek(float)` | se placer dans le morceau (0 à 1) |
| `claude-decide(string, string)` | `(prompt-id, "allow" \| "deny" \| "ask")` |
| `claude-focus(string)` | amène au premier plan le terminal de la session (`id`) |
| `open-url(string)` | ouvre un lien `https://` (ex. `join-url`) |
| `shelf-open(int)`, `shelf-remove(int)` | ouvre / retire le fichier d'index donné |
| `recent-open(string)` | rouvre la conversation d'identifiant donné |
| `timer-action(string)` | `"start:<minutes>"`, `"pause"`, `"resume"`, `"reset"` |

## Exemple

```slint
export component View inherits Window {
    in property <string> time-text;
    in property <bool> has-media;
    in property <string> media-title;
    in property <string> media-artist;
    in property <float> media-progress;
    in property <color> accent;
    in property <[{ title: string, time: string }]> agenda-rows;
    callback media-action(string);

    VerticalLayout {
        padding: 14px;
        spacing: 6px;

        Text { text: root.time-text; color: root.accent; font-size: 20px; font-weight: 700; }
        Text {
            text: root.has-media ? root.media-title + " · " + root.media-artist : "Rien ne joue";
            color: white;
        }
        Rectangle {
            height: 4px;
            background: #ffffff30;
            Rectangle { width: parent.width * root.media-progress; background: root.accent; }
        }
        for item in root.agenda-rows: Text {
            text: item.time + "  " + item.title;
            color: #ffffffb0;
        }
        Rectangle {
            height: 28px;
            background: root.accent;
            border-radius: 14px;
            Text { text: "⏭"; color: black; }
            TouchArea { clicked => { root.media-action("next"); } }
        }
    }
}
```

## Limites

- La vue remplace **tout** le contenu ouvert : à toi d'afficher ce que tu veux
  garder (heure, musique, agenda…). La pilule compacte, elle, ne change pas.
- Les animations de ta vue restent à ta charge : comme pour l'île, ne mets pas
  d'animation en boucle si tu veux garder ~0 % de CPU au repos.
