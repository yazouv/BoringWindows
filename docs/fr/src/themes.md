# Thèmes

Un thème donne d'un coup les couleurs, le contour, la police et l'arrondi de
l'île. Quatre sont fournis, plus un thème automatique :

| Nom | Aspect |
|---|---|
| `auto` | suit Windows : `light` en mode clair, `default` en mode sombre |
| `default` | noir, accent orange (l'île classique) |
| `light` | clair, accent bleu, fin contour : pour les fonds clairs |
| `midnight` | bleu nuit, accent lavande |
| `glass` | verre fumé : légèrement transparent, contour clair, fond flouté |

![Les thèmes default, light, midnight et glass](images/themes.png)

## Choisir un thème

**Réglages… › Apparence › Thème.** L'île change tout de suite. Choisir un
thème remplace les couleurs que tu avais personnalisées par celles du thème.

Ou dans `config.toml` :

```toml
[theme]
name = "midnight"
```

## Ajuster un thème

Les clés écrites dans `[theme]` passent **devant** celles du thème. Pour garder
« midnight » avec un accent vert :

```toml
[theme]
name = "midnight"
accent = "#30D158"
```

Les tailles de l'île, la durée des animations et le décalage depuis le haut de
l'écran ne font pas partie des thèmes : ils se règlent toujours dans `[theme]`
(voir la [référence](configuration.md#theme)).

## Accent de Windows et flou

- **Réglages… › Apparence › « Utiliser la couleur d'accent de Windows »**
  (`system_accent = true`) : l'accent suit celui choisi dans les paramètres de
  Windows, en plus clair sur les thèmes sombres et en plus foncé sur les
  thèmes clairs. Il change en même temps que Windows. La pochette reste
  prioritaire quand la musique joue (onglet Musique).
- **« Flouter ce qui est derrière l'île »** (`blur = true`, activé dans
  `glass`) : ce qui est derrière l'île est flouté, comme les menus de Windows
  11. Le flou épouse la forme exacte de la pilule, animations comprises. Il ne
  se voit qu'à travers un fond translucide (`background` avec une opacité
  inférieure à `FF`). Il faut Windows 11 avec les effets de transparence
  activés (Paramètres › Personnalisation › Couleurs).

```toml
[theme]
name = "auto"
system_accent = true
blur = true
background = "#1C1C1EB8"
```

## Créer son thème

1. **Réglages… › Apparence › Dossier des thèmes** (le dossier `themes`, à côté
   de `config.toml`, est créé s'il n'existe pas).
2. Crée un fichier texte `mon-theme.toml` (le nom du fichier est le nom du
   thème) :

   ```toml
   background = "#2B1B3DF0"   # fond, avec un peu de transparence
   foreground = "#F5EFFF"     # texte
   accent = "#FF7AC6"         # urgent, boutons, réunion imminente
   border = "#FFFFFF26"       # contour (transparent : aucun)
   font = "Segoe UI Variable" # police installée sur Windows (vide : celle du système)
   corner_radius = 24.0       # coins de l'île ouverte
   ```

   Toutes les clés sont facultatives : ce qui manque prend la valeur du thème
   `default`.
3. Il apparaît dans la liste des thèmes (rouvre la fenêtre de réglages), ou
   mets `name = "mon-theme"` dans `[theme]`.

Le fichier est surveillé : enregistre-le et l'île se met à jour. Une erreur
(clé inconnue, couleur mal écrite) s'affiche dans l'île, comme pour
`config.toml`.

Les couleurs s'écrivent `"#RRGGBB"` ou `"#RRGGBBAA"` (les deux derniers
chiffres : opacité, de `00` transparent à `FF` opaque). Le texte secondaire,
les boutons et les barres sont calculés à partir de `foreground` et `accent`.

Pour partager un thème, il suffit d'envoyer son fichier `.toml`.
