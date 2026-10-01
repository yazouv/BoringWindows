# BoringWindows

Une « Dynamic Island » pour Windows, écrite en Rust : légère, discrète et entièrement customisable.

- 🤖 Statut **Claude Code** (travaille / a besoin de toi / terminé) via hooks, avec réponse aux demandes de permission
- 🎵 **Musique en cours** (Spotify, Apple Music, navigateur…) avec précédent / pause / suivant
- 📅 **Calendrier** (ICS, Google, Outlook, CalDAV) avec rappel et bouton « Rejoindre »
- 🎨 Thèmes, layouts et modules configurables par fichiers, rechargés à chaud

Inspiré de [boring.notch](https://github.com/TheBoredTeam/boring.notch) et [coucou](https://github.com/Louis-CFM/coucou).

👉 Voir le [plan du projet](PLAN.md).

## État

Phase 0 (fondations) : l'île s'affiche en haut de l'écran, s'ouvre au survol, et
change de forme quand un module réclame l'attention. Pas encore de vrais modules :
un module `demo` permet de tester les animations.

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
| `bw-app` | binaire : île Slint (`ui/island.slint`), intégration Win32, icône de notification |

Le code Win32 est isolé dans `crates/bw-app/src/platform/win32.rs` ; hors Windows,
l'île s'ouvre comme une fenêtre normale, ce qui suffit pour travailler l'UI.
