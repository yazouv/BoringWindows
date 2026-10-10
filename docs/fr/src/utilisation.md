# Utiliser l'île

## Les trois états

| État | Quand | Ce que tu vois |
|---|---|---|
| **Compacte** | rien à signaler | une petite pilule noire |
| **Attention** | un module a quelque chose à dire | une pilule plus large avec un point et un texte court |
| **Ouverte** | souris sur l'île (ou clic) | l'heure, la musique, l'agenda, les sessions Claude |

![Compacte, attention (musique), attention urgente (Claude), ouverte](images/etats.png)

Le point de la pilule est **orange** quand c'est urgent (Claude attend une
réponse), gris sinon. Quand la musique a la main, la pochette remplace le point.

Si plusieurs choses arrivent en même temps, la plus importante gagne : une
question de Claude passe avant une réunion qui commence, qui passe avant la
musique. L'ordre à importance égale se règle avec
[`layout.compact`](configuration.md#layout).

Quand la musique et Claude tournent en même temps, une **bulle** se détache à
droite de la pilule pour celui qui n'a pas la main : la pastille de Claude
(irisée quand il travaille, « ! » quand il attend, coche quand il a fini) ou
la pochette du morceau. Un clic dessus ouvre l'île.

## Les onglets

L'île ouverte a des onglets, en haut à droite :

- **Accueil** : la musique, l'agenda, les sessions Claude, le minuteur,
  l'étagère ;
- **Notifs** : les dernières [notifications](notifications.md) des
  applications, avec un badge pour celles que tu n'as pas vues ;
- **Claude** : la consommation et les
  [conversations récentes](claude-code.md#conversations-récentes-et-consommation).

Changer d'onglet fait glisser le contenu. Un onglet n'apparaît que si son module est actif. L'île revient sur l'accueil
quand elle se referme.

## Ouvrir et fermer

- **Survole** l'île pour l'ouvrir ; elle se referme quand la souris s'en va.
- Tu préfères cliquer ? Mets `open_on = "click"` dans
  [`[general]`](configuration.md#general).
- Seule la pilule réagit à la souris : le reste de la bande en haut de l'écran
  laisse passer les clics vers les fenêtres en dessous.
- L'île ne prend jamais le focus : ton clavier reste dans l'application où tu
  tapes.

## Le menu de l'icône

Clic droit sur l'icône BoringWindows dans la zone de notification :

| Entrée | Effet |
|---|---|
| Réglages… | ouvre la fenêtre de réglages (voir plus bas) |
| Ouvrir la configuration | ouvre `config.toml` dans ton éditeur |
| Recharger la configuration | relit le fichier (utile seulement si le rechargement automatique a raté) |
| Claude Code : installer / retirer les hooks… | branche Claude Code (voir [Claude Code](claude-code.md)) |
| Lancer au démarrage | démarre BoringWindows avec Windows |
| Masquer l'île | la cache sans quitter |
| Quitter | ferme BoringWindows |

## Plein écran

Quand une application passe en plein écran (jeu, vidéo F11, présentation) sur
le même écran, l'île se cache, puis revient à la sortie du plein écran.
Désactivable avec `hide_in_fullscreen = false`.

Les outils de capture d'écran (Win+Maj+S, Outil Capture d'écran, ShareX,
Greenshot…) couvrent aussi tout l'écran, mais l'île reste affichée pendant la
capture.

L'île reste au-dessus de la barre des tâches, même quand celle-ci est placée en
haut de l'écran. Si cette barre est plus basse que la pilule (petites icônes),
la pilule fermée et celle d'attention prennent sa hauteur pour ne pas dépasser ;
avec une barre de taille normale, rien ne change.

## Modifier la configuration

Le plus simple : clic droit sur l'icône › **Réglages…**. Une page par sujet,
choisie dans la colonne de gauche (Général, Apparence, Notifications, Claude
Code, Agenda, Musique…), modifie les réglages
courants ; **chaque changement est enregistré et appliqué tout de suite**, pas
de bouton « Valider ». C'est aussi là qu'on ajoute un calendrier (assistant par
service, bouton **Tester**), qu'on installe les hooks Claude Code et qu'on
lance le diagnostic. La fenêtre peut aussi s'ouvrir au lancement :
`boringwindows.exe --settings`.

La langue de l'app (français ou anglais) suit celle de Windows ; elle se
change dans **Réglages… › Général › Langue**, sans redémarrer.

La fenêtre écrit dans `config.toml` en gardant tes commentaires et l'ordre du
fichier. Pour le reste (tailles de l'île, ordre des modules…), édite le
fichier directement.

Tout se règle dans `%APPDATA%\BoringWindows\config.toml`. **Enregistre le
fichier : l'île se met à jour aussitôt**, sans redémarrage. Si le fichier
contient une erreur, l'île l'affiche (« ⚠ config.toml invalide… ») et garde
les réglages précédents.

Toutes les options sont décrites dans la [référence](configuration.md).
