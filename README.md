# BoringWindows

Une « Dynamic Island » pour Windows, écrite en Rust : légère, discrète et entièrement customisable.

- 🤖 Statut **Claude Code** (travaille / a besoin de toi / terminé) via hooks, avec réponse aux demandes de permission
- 🎵 **Musique en cours** (Spotify, Apple Music, navigateur…) avec précédent / pause / suivant
- 📅 **Calendrier** (ICS, Google, Outlook, CalDAV) avec rappel et bouton « Rejoindre »
- 🎨 Thèmes, layouts et modules configurables par fichiers, rechargés à chaud

Inspiré de [boring.notch](https://github.com/TheBoredTeam/boring.notch) et [coucou](https://github.com/Louis-CFM/coucou).

👉 Voir le [plan du projet](PLAN.md).

## État

- **Phase 0** (fondations) : l'île s'affiche en haut de l'écran, s'ouvre au survol
  et change de forme quand un module réclame l'attention.
- **Phase 1** (Claude Code) : statut des sessions en direct, et réponse aux
  demandes de permission depuis l'île.
- **Phase 2** (musique) : morceau en cours, pochette, précédent / lecture / suivant,
  barre de progression cliquable, île teintée à la couleur de la pochette.

## Musique

Rien à configurer : BoringWindows lit les contrôles média de Windows, ceux de
l'overlay de volume. Ça couvre Spotify, Apple Music, Deezer, les navigateurs
(YouTube, SoundCloud…), VLC, le lecteur Windows… sans clé d'API ni connexion.

- Pilule : pochette + `♪ titre — artiste` tant que la musique joue.
- Île ouverte : pochette, titre, artiste, ⏮ ⏯ ⏭, barre de progression (clic pour
  se déplacer dans le morceau).
- Plusieurs lecteurs ouverts : celui qui joue est affiché ; clic sur le nom de la
  source (en haut à droite, `⇄`) pour passer au suivant.
- `[modules.media]` : `accent_from_artwork`, `ignore = ["msedge"]`…

## Claude Code

1. Lance BoringWindows, puis clic droit sur l'icône de notification →
   **Claude Code : installer les hooks…**. La boîte de dialogue liste ce qui sera
   ajouté à `~/.claude/settings.json` ; une sauvegarde datée du fichier est faite
   avant, et seules nos entrées sont touchées (le même menu les retire).
2. Relance tes sessions Claude Code (les hooks sont lus au démarrage).

Ce que montre l'île :

| Pilule | Signification |
|---|---|
| point gris · `projet · Bash` | Claude travaille |
| point orange · `projet · autoriser Bash ?` | demande de permission : survole l'île pour **Autoriser / Refuser / Terminal** |
| point orange · `projet · attend ta réponse` | Claude attend dans le terminal |
| `projet · terminé` | fin de tour (quelques secondes) |

Dans l'île ouverte, un clic sur une session ramène son terminal au premier plan.
Sans réponse dans l'île au bout de `permission_wait_secs` (60 s par défaut), la
question repasse dans le terminal.

**Sécurité** : le relais (`boringwindows hook`, copié dans
`%LOCALAPPDATA%\BoringWindows\bin\bw-hook.exe`) n'envoie qu'un résumé (projet,
outil, commande ou nom de fichier, jamais le contenu des fichiers ni tes
prompts) sur un named pipe local réservé à ton compte. Si l'app est fermée ou ne
répond pas en 300 ms, il sort sans rien faire : Claude n'est jamais bloqué.

## Lancer

```powershell
cargo run -p bw-app            # debug, avec logs dans la console
cargo build --release          # target\release\boringwindows.exe
pwsh scripts/measure-idle.ps1  # RAM / CPU au repos
```

Au premier lancement, `%APPDATA%\BoringWindows\config.toml` est créé avec toutes
les options commentées. Il est rechargé dès que tu l'enregistres. Pour voir les
animations sans vrais modules :

```toml
[modules.demo]
enabled = true
```

Clic droit sur l'icône de la zone de notification : ouvrir la config, lancer au
démarrage, masquer l'île, quitter.

## Structure

| Crate | Rôle |
|---|---|
| `bw-core` | trait `Module`, hôte des modules (tokio), arbitrage d'attention |
| `bw-config` | `config.toml` : schéma, validation, rechargement à chaud |
| `bw-claude` | relais de hooks, protocole IPC, suivi des sessions, installation dans `settings.json` |
| `bw-media` | musique : contrôles média de Windows (GSMTC), pochette, couleur d'accent |
| `bw-app` | binaire : île Slint (`ui/island.slint`), intégration Win32, icône de notification |

Le code Win32 est isolé dans `crates/bw-app/src/platform/win32.rs`. Sur macOS et
Linux, l'île s'ouvre comme une fenêtre flottante (macOS : icône de barre de menus,
pas d'icône dans le Dock) ; le port macOS complet est décrit dans le plan.

La CI construit un `.exe` Windows et un binaire macOS (Apple Silicon) à chaque
push : onglet *Actions* → run → *Artifacts*.
