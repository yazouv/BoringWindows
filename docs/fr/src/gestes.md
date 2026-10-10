# Gestes

Quelques gestes sur l'île, pour aller vite :

| Geste | Effet |
|---|---|
| Molette vers le haut / le bas | monter / baisser le volume (2 % par cran) |
| Glisser vers la gauche sur le pavé tactile, molette inclinée à droite, ou Maj + molette | morceau suivant |
| Glisser vers la droite (ou dans l'autre sens) | morceau précédent |
| Cliquer-glisser horizontalement à la souris sur l'île | morceau suivant ou précédent |
| Appui long (un peu plus d'une demi-seconde) | activer ou couper « ne pas déranger » |

L'île confirme chaque geste : « Volume 46 % », « Morceau suivant »… dans
l'en-tête si elle est ouverte, dans la pilule sinon. Si le module
[volume](volume.md) est activé, c'est son propre affichage qui montre le
volume.

Réglages › **Général** › « Gestes », ou dans `config.toml` :

```toml
[modules.gestures]
enabled = true
volume_step = 2   # % par cran de molette (1 à 10)
```

Windows seulement pour le volume ; les morceaux passent par les contrôles
média, comme les boutons du lecteur.
