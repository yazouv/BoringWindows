# Processeur, mémoire et raccourcis

## Processeur et mémoire

Dans l'île ouverte, à côté de l'heure (et de la météo), deux petits compteurs
donnent l'usage du **processeur** et de la **mémoire**. **Survole-les** : la
date laisse la place à la mémoire utilisée, par exemple « Mémoire 8,4 /
16,0 Go ». Une valeur au-dessus de 90 % passe en rouge.

Rien n'est mesuré quand l'île est fermée : la mesure tourne seulement pendant
qu'elle est ouverte (toutes les 2 secondes par défaut).

### Alertes

Tu peux être prévenu quand le processeur ou la mémoire **reste** chargé :
après 30 secondes au-dessus du seuil, la pilule affiche par exemple
« Processeur à 97 % · chrome », avec l'application qui consomme le plus.
L'alerte ne revient qu'une fois la charge nettement redescendue (10 points
sous le seuil). Avec une alerte réglée, une mesure est prise toutes les
5 secondes, île fermée comprise.

Réglages › **Système** : active ou désactive l'affichage, choisis le
rafraîchissement et les seuils d'alerte. Ou dans `config.toml` :

```toml
[modules.system]
enabled = true
refresh_secs = 2          # 1 à 10, île ouverte
cpu_alert_percent = 90    # 0 = pas d'alerte, sinon 50 à 100
ram_alert_percent = 0
alert_after_secs = 30     # 5 à 600
```

## Raccourcis clavier

Des raccourcis valables dans toutes les applications :

| Action | Clé de config | Par défaut |
|---|---|---|
| Ouvrir ou refermer l'île | `toggle` | `Ctrl+Alt+B` |
| Lecture / pause | `play_pause` | aucun |
| Morceau suivant | `next_track` | aucun |
| Morceau précédent | `previous_track` | aucun |
| Ne pas déranger | `do_not_disturb` | aucun |

Ouverte au clavier, l'île se referme seule au bout de 6 secondes si la souris
ne vient pas dessus (ou au prochain appui sur le raccourci).

Réglages › **Système** › *Raccourcis clavier*, ou dans `config.toml` :

```toml
[hotkeys]
toggle = "Ctrl+Alt+B"
play_pause = "Ctrl+Alt+P"
next_track = "Ctrl+Alt+Right"
previous_track = "Ctrl+Alt+Left"
do_not_disturb = ""          # vide : pas de raccourci
```

Un raccourci s'écrit avec ses modificateurs d'abord, séparés par `+` :

- **Modificateurs** : `Ctrl`, `Alt`, `Shift`, `Super` (touche Windows ; `Cmd`
  sur Mac). Au moins un est obligatoire, sauf pour `F13` à `F24`, `Pause` et
  `ScrollLock`.
- **Touches** : `A` à `Z`, `0` à `9`, `F1` à `F24`, `Space`, `Enter`, `Tab`,
  `Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PageUp`, `PageDown`,
  `Insert`, `Delete`, `Num0` à `Num9`…

Si un raccourci est déjà pris par une autre application, ou mal écrit, l'île
l'affiche (`⚠ …`) et les autres raccourcis restent actifs.

> Sous Linux, les raccourcis passent par X11 : ils ne marchent pas dans une
> session Wayland pure.
