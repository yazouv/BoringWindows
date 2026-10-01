# Étagère

Glisse des fichiers sur l'île pour les garder sous la main, comme sur le bureau
d'à côté : le temps d'un mail, d'un dépôt de devoir ou d'un transfert.

## Utilisation

1. Glisse un ou plusieurs fichiers (depuis l'Explorateur, un navigateur…) vers
   la pilule : elle s'ouvre toute seule pendant le survol.
2. Relâche : une ligne **Étagère** apparaît avec le nom du fichier.
3. **Clic** sur un fichier : il s'ouvre avec son application habituelle.
   **×** : le retire de l'étagère (le fichier, lui, n'est jamais touché).

Seul le **chemin** est gardé, pas une copie : si le fichier est déplacé ou
supprimé, il disparaît de l'étagère. La liste survit au redémarrage (fichier
`shelf.txt` à côté de `config.toml`).

L'île affiche les 4 derniers fichiers (« +3 » indique les autres). Les plus
anciens sortent quand la limite est atteinte.

## Réglages

Réglages › **Étagère**, ou dans `config.toml` :

```toml
[modules.shelf]
enabled = true   # false : l'île ne réagit plus aux dépôts
max = 8          # 1 à 30 fichiers gardés
```

Le glisser-déposer *depuis* l'île vers une autre application n'est pas géré :
clique sur le fichier pour l'ouvrir, puis utilise l'application voulue.
