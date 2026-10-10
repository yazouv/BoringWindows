# Mascotte

Un petit blob à yeux vit dans la pilule. Il change d'humeur selon ce qui se
passe :

| Moment | La mascotte |
|---|---|
| Rien de spécial | au centre de la pilule, tranquille |
| Le matin (6 h à 11 h) | yeux rieurs |
| Le soir (21 h à 6 h) | yeux mi-clos, elle a sommeil |
| Personne au clavier depuis 10 min | elle dort (« z »), et se réveille dès que tu touches au clavier ou à la souris |
| Claude Code travaille | elle s'affaire, les yeux qui vont et viennent |
| Claude t'attend (permission, question) | elle sautille, toute colorée, avec un « ! » |
| Claude a fini | elle sourit et saute de joie |
| La musique joue | elle danse sur les basses, à côté de la pochette, et la pilule respire en rythme |

Quand Claude occupe la pilule, la mascotte prend la place du point de statut ;
quand c'est la musique, elle se met à droite.

## Saisons

Elle se déguise, et l'île ouverte se décore :

- **Halloween** (tout octobre, jusqu'au 1er novembre) : chapeau de sorcière et
  petites citrouilles ;
- **Noël** (1er au 26 décembre) : bonnet de Noël et flocons ;
- **Nouvel An** (30 décembre au 2 janvier) : chapeau de fête et confettis.

## Réglages

Réglages › **Mascotte**, ou dans `config.toml` :

```toml
[modules.mascot]
enabled = true
always_animated = false   # toujours animée (respire, cligne des yeux)
music = true              # danser sur la musique
seasonal = true           # déguisements et décorations
sleep_after_minutes = 10  # avant qu'elle s'endorme (1 à 240)
```

Par défaut, la mascotte ne bouge **que quand il se passe quelque chose**
(Claude, musique) : au repos, elle est figée et l'île ne consomme rien.
`always_animated = true` la fait respirer et cligner des yeux en permanence,
au prix d'un peu de CPU.

La danse utilise la capture audio du [visualiseur](configuration.md#modulesvisualizer),
seulement pendant que la musique joue.
