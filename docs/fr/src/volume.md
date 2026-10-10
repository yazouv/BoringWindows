# Volume et luminosité

Chaque changement du volume du système (touches du clavier, mélangeur Windows,
une application) s'affiche un instant dans la pilule : « Volume 45 % » ou
« Muet ». Désactivé par défaut, car Windows affiche déjà son propre indicateur.

Active-le dans Réglages › **Notifications** › « Volume », ou dans
`config.toml` :

```toml
[modules.volume]
enabled = true
show_secs = 2   # durée d'affichage après un changement (1 à 10)
```

Ce que ça fait, et ne fait pas :

- Le module écoute le périphérique de sortie **par défaut** ; il ne consomme
  rien au repos (callback de l'API audio de Windows, pas de polling).
- Il **n'enlève pas** l'indicateur de Windows : les deux s'affichent.
- Si tu changes de périphérique de sortie par défaut en cours de route,
  redémarre BoringWindows (ou recharge les modules en modifiant `config.toml`).
- Windows seulement.

## Luminosité

Même principe pour la luminosité de l'écran : « Luminosité 70 % » s'affiche à
chaque changement (touches du clavier, centre de notifications, économiseur de
batterie). Désactivé par défaut lui aussi.

Active-le dans Réglages › **Notifications** › « Luminosité », ou dans
`config.toml` :

```toml
[modules.brightness]
enabled = true
show_secs = 2   # durée d'affichage après un changement (1 à 10)
```

- Seul l'**écran intégré** d'un portable ou d'une tablette publie sa luminosité
  à Windows. Sur un PC fixe ou un écran externe, le module ne fait rien (le
  journal indique « aucun écran intégré »).
- L'écoute passe par un événement WMI (`WmiMonitorBrightnessEvent`) : le
  thread dédié se réveille au plus toutes les 2 secondes pour vérifier si le
  module doit s'arrêter, sans autre coût au repos.
