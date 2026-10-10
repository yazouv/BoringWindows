# Mode présentation

Pendant un appel ou un partage d'écran, l'île se fait discrète :

- **aucune notification ne s'annonce** (elles arrivent quand même dans
  l'onglet « Notifs ») ;
- un **point rouge** s'affiche dans la pilule, pour te rappeler que tu es en
  direct.

Et, que tu sois en appel ou non, **l'île n'apparaît jamais dans les partages
d'écran, enregistrements et captures** : tes notifications ne fuient pas
pendant une démo. Attention, ça vaut aussi pour tes propres captures
(Win + Maj + S) : décoche l'option si tu veux faire une capture de l'île.

## Comment c'est détecté

Windows n'a pas d'API « écran partagé », mais il note, application par
application, qui utilise le micro et qui capture l'écran (c'est ce qui
alimente l'icône de micro de la barre des tâches). L'île considère que tu es
en direct quand :

- une application **capture l'écran** (Teams, Discord, Zoom, OBS… avec la
  capture de Windows) ;
- ou une **application d'appel** utilise le micro (liste `call_apps`, par
  défaut Teams, Discord, Zoom, Slack, Webex, Skype, les navigateurs pour Meet,
  OBS).

La détection réagit aux changements (aucune interrogation en boucle). Une
application qui plante laisse parfois son entrée « en cours » : l'île vérifie
que l'application tourne encore.

## Réglages

Réglages › **Notifications** › « Mode présentation » et « Cacher des
captures », ou dans `config.toml` :

```toml
[modules.presentation]
enabled = true            # détection des appels et partages d'écran
hide_from_capture = true  # l'île n'apparaît pas dans les captures
call_apps = ["teams", "discord", "zoom", "slack", "webex", "skype", "chrome", "msedge", "firefox", "brave", "opera", "obs"]
```

Windows seulement.
