# Installation

## Télécharger

BoringWindows tient en un seul fichier. Va sur la page
**[Releases](https://github.com/yazouv/BoringWindows/releases/latest)** et
télécharge celui de ton système :

| Système | Fichier |
|---|---|
| Windows 10/11 | `boringwindows-windows-x64.exe` |
| macOS (Apple Silicon) | `boringwindows-macos-arm64.tar.gz` |
| macOS (Intel) | `boringwindows-macos-x64.tar.gz` |
| Linux (x64) | `boringwindows-linux-x64.tar.gz` |

**Windows** : renomme le fichier en `boringwindows.exe` si tu veux et range-le
où tu veux, par exemple dans `C:\Users\<toi>\Apps\BoringWindows\` (un
dossier à toi : la mise à jour automatique doit pouvoir le remplacer).

> Windows peut afficher un avertissement SmartScreen (« application non
> reconnue ») tant que l'exécutable n'est pas signé : *Informations
> complémentaires* › *Exécuter quand même*.

**macOS** : décompresse l'archive, puis autorise ce binaire non signé :

```sh
xattr -d com.apple.quarantine boringwindows
./boringwindows
```

Sur Mac, l'île se pose au-dessus de la barre de menus, centrée en haut de
l'écran (autour de l'encoche s'il y en a une), et prend la hauteur de la
barre de menus quand elle est fermée. Un clic sur l'île ne retire pas le
focus à l'application en cours, et les clics à côté de la pilule passent à
travers. L'icône est dans la barre de menus, pas dans le Dock.
**Démarrer avec le système** ajoute un LaunchAgent
(`~/Library/LaunchAgents/io.github.yazouv.boringwindows.plist`). Pas encore
sur Mac : le flou (`theme.blur`), les notifications des applications, la
batterie, le Bluetooth, le volume et le mode présentation ; et l'île se cache
toujours dans les applications en plein écran.

**Linux** : `tar -xzf boringwindows-linux-x64.tar.gz && ./boringwindows`
(l'île s'ouvre comme une fenêtre flottante, sans icône de notification).

## Mises à jour

BoringWindows vérifie les nouvelles versions au lancement puis toutes les
6 heures. Quand il en trouve une, il la télécharge, vérifie son empreinte
(SHA-256) et remplace son exécutable ; l'île affiche « BoringWindows x.y.z
installé ». Elle s'applique au prochain lancement, ou tout de suite avec
clic droit sur l'icône › **Redémarrer pour passer à x.y.z**.

- Désactiver : **Réglages… › Général › Installer les mises à jour
  automatiquement** (`auto_update = false` dans `[general]`).
- Vérifier à la main : **Réglages… › Général › Rechercher une mise à jour**, ou
  le même élément dans le menu de l'icône.
- Seule requête envoyée : la liste des releases à `api.github.com`, puis le
  téléchargement du fichier.
- Version compilée depuis un fork **privé** : GitHub ne montre ses releases
  qu'aux comptes autorisés. Mets un jeton en lecture seule (*Fine-grained
  token*, permission *Contents: Read*) dans la variable d'environnement
  `BORINGWINDOWS_GITHUB_TOKEN`. Inutile pour le dépôt public.
- Une version compilée avec `cargo` ne se met pas à jour toute seule.

## Ou compiler toi-même

Il faut [Rust](https://rustup.rs) et les outils de compilation C++ de Visual
Studio (proposés par l'installeur de Rust).

```powershell
git clone https://github.com/yazouv/BoringWindows
cd BoringWindows
cargo run --release
```

La première compilation prend quelques minutes, les suivantes quelques secondes.

## Premier lancement

Lance `boringwindows.exe` : une pilule noire apparaît en haut au centre de
l'écran, et une icône s'ajoute dans la zone de notification (en bas à droite,
près de l'horloge ; regarde dans la flèche ^ si tu ne la vois pas).

Au premier lancement, le fichier de configuration est créé avec toutes les
options commentées :

```text
%APPDATA%\BoringWindows\config.toml
```

Tu n'as rien à y modifier pour commencer : la musique marche tout de suite.
Pour Claude Code et l'agenda, suis les pages dédiées.

## Lancer au démarrage de Windows

Clic droit sur l'icône › **Lancer au démarrage**.

## Désinstaller

1. Si tu as branché Claude Code : clic droit sur l'icône › **Claude Code :
   retirer les hooks…**.
2. Décoche **Lancer au démarrage**, puis **Quitter**.
3. Supprime `boringwindows.exe`, et si tu veux les dossiers
   `%APPDATA%\BoringWindows` (configuration) et `%LOCALAPPDATA%\BoringWindows`
   (relais Claude, journaux).
