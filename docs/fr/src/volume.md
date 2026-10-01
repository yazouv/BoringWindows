# Volume

Chaque changement du volume du système (touches du clavier, mélangeur Windows,
une application) s'affiche un instant dans la pilule : « Volume 45 % » ou
« Muet ». Désactivé par défaut, car Windows affiche déjà son propre indicateur.

Active-le dans Réglages › **Général** › « Afficher les changements de volume
dans l'île », ou dans `config.toml` :

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
- Windows seulement. La luminosité n'est pas gérée.
