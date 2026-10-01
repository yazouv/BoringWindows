# Installation

## Télécharger

BoringWindows tient en un seul fichier, `boringwindows.exe`.

1. Ouvre l'onglet **[Actions](https://github.com/yazouv/BoringWindows/actions)**
   du dépôt et clique sur le dernier passage réussi (✅).
2. En bas de la page, section **Artifacts**, télécharge
   `boringwindows-windows-x64` (il faut être connecté à GitHub), puis
   décompresse-le.
3. Range `boringwindows.exe` où tu veux, par exemple dans
   `C:\Users\<toi>\Apps\BoringWindows\`.

> Windows peut afficher un avertissement SmartScreen (« application non
> reconnue ») tant que l'exécutable n'est pas signé : *Informations
> complémentaires* › *Exécuter quand même*.

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
