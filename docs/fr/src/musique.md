# Musique

**Rien à configurer.** BoringWindows lit les mêmes informations que l'overlay
de volume de Windows : tout lecteur qui s'y affiche s'affiche aussi dans l'île.
Spotify, Apple Music, Deezer, Tidal, YouTube et SoundCloud dans le navigateur,
VLC, le lecteur multimédia de Windows… sans compte ni clé d'API.

![Le lecteur dans l'île ouverte, puis la pochette dans la pilule](images/musique.png)

## Ce que tu vois

- **Pilule** : la pochette et `♪ titre — artiste` tant que la musique joue.
- **Île ouverte** : pochette, titre, artiste, boutons **précédent / lecture /
  suivant** et barre de progression. **Clique sur la barre** pour avancer ou
  reculer dans le morceau.
- **Couleur** : l'île prend la teinte dominante de la pochette (boutons,
  barre), éclaircie pour rester lisible sur fond noir.

Un bouton grisé signifie que le lecteur ne propose pas cette action (certains
sites ne permettent pas de se déplacer dans le morceau, par exemple).

## Plusieurs lecteurs

S'il y a plusieurs lecteurs ouverts, c'est **celui qui joue** qui est affiché.
En haut à droite de l'île, le nom de la source est suivi de `⇄` : clique dessus
pour passer au lecteur suivant.

Pour ne jamais afficher un lecteur (le navigateur du travail, par exemple) :

```toml
[modules.media]
ignore = ["msedge"]
```

Il suffit d'un morceau du nom : `"chrome"`, `"msedge"`, `"firefox"`,
`"spotify"`… Les noms exacts sont affichés dans la console
(`musique : sources [...]`) si tu lances BoringWindows depuis un terminal.

## Réglages

```toml
[modules.media]
enabled = true               # false pour désactiver complètement
accent_from_artwork = true   # teinter l'île avec la pochette
ignore = []
```

## Visualiseur audio (optionnel)

De petites barres d'équaliseur animées dans l'en-tête de l'île, d'après le son
réellement joué par le système. C'est la **seule fonction qui travaille en
continu**, donc elle est éteinte par défaut ; la capture ne tourne que lorsque
l'île est ouverte **et** qu'une musique joue, et s'arrête dès que l'une des deux
conditions tombe.

Active-la dans Réglages › **Musique**, ou dans `config.toml` :

```toml
[modules.visualizer]
enabled = true
bands = 12   # nombre de barres (4 à 32)
fps = 30     # images par seconde (10 à 60)
```

Windows seulement (capture « loopback » du périphérique de sortie par défaut).
Elle capte tout ce que le système joue, pas seulement le lecteur affiché.
