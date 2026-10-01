# BoringWindows

Une « Dynamic Island » pour Windows, écrite en Rust : légère, discrète et entièrement customisable.

- 🤖 Statut **Claude Code** (travaille / a besoin de toi / terminé) via hooks, avec réponse aux demandes de permission
- 🎵 **Musique en cours** (Spotify, Apple Music, navigateur…) avec précédent / pause / suivant
- 📅 **Calendrier** (ICS, Google, Outlook, CalDAV) avec rappel et bouton « Rejoindre »
- 🎨 Thèmes, layouts et modules configurables par fichiers, rechargés à chaud

Inspiré de [boring.notch](https://github.com/TheBoredTeam/boring.notch) et [coucou](https://github.com/Louis-CFM/coucou).

📖 **Documentation : [yazouv.github.io/BoringWindows](https://yazouv.github.io/BoringWindows/)**
(installation, configuration de l'agenda pour Google / Outlook / iCloud…,
Claude Code, dépannage) · 🇬🇧 [English](https://yazouv.github.io/BoringWindows/en/).
Sources dans [`docs/`](docs/).

👉 Voir le [plan du projet](PLAN.md).

## État

- **Phase 0** (fondations) : l'île s'affiche en haut de l'écran, s'ouvre au survol
  et change de forme quand un module réclame l'attention.
- **Phase 1** (Claude Code) : statut des sessions en direct, et réponse aux
  demandes de permission depuis l'île.
- **Phase 2** (musique) : morceau en cours, pochette, précédent / lecture / suivant,
  barre de progression cliquable, île teintée à la couleur de la pochette.
- **Phase 3** (agenda) : prochaines réunions depuis tes calendriers ICS, rappel
  avant le début, bouton « Rejoindre » (Teams, Meet, Zoom, Webex…).
- **Phase 3.5** (réglages) : fenêtre de réglages (clic droit sur l'icône ›
  **Réglages…**) : plus besoin d'éditer `config.toml`, assistant d'ajout de
  calendrier avec bouton **Tester**, hooks et diagnostic Claude Code.
- **Phase 4** (en cours) : thèmes (`default`, `light`, `midnight`, `glass` ou
  les tiens dans `themes/<nom>.toml`), au choix dans Réglages › Apparence.
- **Langues** : l'app est en français ou en anglais (celle de Windows par
  défaut, `general.language` pour forcer), changement à chaud.

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

## Agenda

Le plus simple : clic droit sur l'icône › **Réglages…** › **Agenda**, choisis
ton service, colle le lien, **Tester**, **Ajouter**. Ou, à la main, ajoute le
lien ICS privé de chaque calendrier dans `config.toml` :

```toml
[[modules.calendar.sources]]
name = "Pro"
url = "https://outlook.office365.com/owa/calendar/…/calendar.ics"

[[modules.calendar.sources]]
name = "Perso"
url = "https://calendar.google.com/calendar/ical/…/basic.ics"
```

Où trouver ce lien :
- **Google Agenda** : Paramètres › (ton agenda) › *Intégrer l'agenda* › « Adresse
  secrète au format iCal ».
- **Outlook / Microsoft 365** : Paramètres › Calendrier › Calendriers partagés ›
  *Publier un calendrier* › lien ICS (si ton organisation l'autorise).
- **iCloud** : Partager le calendrier › Calendrier public › copier le lien
  (`webcal://` accepté).

Ce lien donne accès à ton agenda : garde `config.toml` pour toi.

- Île ouverte : les prochains événements (24 h), avec compte à rebours dans
  l'heure qui vient et bouton **Rejoindre** quand l'invitation contient un lien
  Teams, Meet, Zoom, Webex, Whereby…
- Pilule : `Réunion à 14:30` dans l'heure, puis `Réunion dans 4 min` (rappel,
  `remind_minutes`) et `Réunion · a commencé` pendant 10 minutes.
- Récurrences, exceptions, réunions déplacées ou annulées et fuseaux Outlook
  (« Romance Standard Time »…) sont gérés. Rafraîchi toutes les 10 min ; en cas
  de coupure réseau, la dernière version reste affichée.

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
| point orange · `projet · te pose une question` | question ou plan à valider : réponds dans le terminal |
| `projet · terminé` | fin de tour (quelques secondes) |

Dans l'île ouverte, un clic sur une session ramène son terminal au premier plan.
Sans réponse dans l'île au bout de `permission_wait_secs` (60 s par défaut), la
question repasse dans le terminal. Une demande réglée ailleurs (terminal, Échap)
disparaît de l'île dès que Claude passe à la suite (Échap compris). Un son
système signale chaque fois que Claude se met à t'attendre : question, plan,
permission (`sound = false` pour le couper).

**Rien ne s'affiche ?** Lance le diagnostic (BoringWindows ouvert) :

```powershell
cargo run -- doctor          # ou : boringwindows.exe doctor
```

Il vérifie les hooks dans `settings.json`, la copie du relais, que l'app
répond, lance le relais comme le fait Claude Code (via bash et cmd), affiche
les derniers appels du journal (`%LOCALAPPDATA%\BoringWindows\hook.log`) et
fait apparaître « diagnostic » dans l'île. Le rapport est aussi enregistré dans
`%LOCALAPPDATA%\BoringWindows\doctor.txt`.

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

Clic droit sur l'icône de la zone de notification : réglages, ouvrir la config, lancer au
démarrage, masquer l'île, quitter.

## Structure

| Crate | Rôle |
|---|---|
| `bw-core` | trait `Module`, hôte des modules (tokio), arbitrage d'attention |
| `bw-config` | `config.toml` : schéma, validation, rechargement à chaud |
| `bw-claude` | relais de hooks, protocole IPC, suivi des sessions, installation dans `settings.json` |
| `bw-media` | musique : contrôles média de Windows (GSMTC), pochette, couleur d'accent |
| `bw-calendar` | agenda : lecture ICS, récurrences, rappels, liens de visio |
| `bw-i18n` | langue courante et macro `tr!("anglais", "français")` pour les textes côté Rust |
| `bw-app` | binaire : île Slint (`ui/island.slint`), intégration Win32, icône de notification |

Le code Win32 est isolé dans `crates/bw-app/src/platform/win32.rs`. Sur macOS et
Linux, l'île s'ouvre comme une fenêtre flottante (macOS : icône de barre de menus,
pas d'icône dans le Dock) ; le port macOS complet est décrit dans le plan.

Pas de CI de build/tests (elle consommait trop de minutes GitHub Actions) :
avant de pousser, lance `cargo clippy --workspace --all-targets -- -D warnings`
et `cargo test --workspace`. Seule la doc est publiée automatiquement depuis
`main`.
