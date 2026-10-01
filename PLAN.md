# BoringWindows — Plan

> Une « Dynamic Island » pour Windows, en Rust, légère et entièrement customisable.
> Inspirée de [boring.notch](https://github.com/TheBoredTeam/boring.notch) (macOS) et de
> [coucou](https://github.com/Louis-CFM/coucou) (statut Claude Code).

---

## 1. Vision

Une petite pilule ancrée en haut-centre de l'écran qui :

- **reste discrète** (quasi invisible) quand rien ne se passe ;
- **s'anime / grossit** quand un événement mérite l'attention (Claude attend une réponse, nouveau morceau, réunion dans 5 min) ;
- **s'ouvre** au survol ou au clic pour montrer les widgets complets ;
- se **configure entièrement** par fichiers texte (thème, layout, modules) avec rechargement à chaud.

### Objectifs de performance (non négociables)

| Métrique | Cible |
|---|---|
| RAM au repos | < 30 Mo |
| CPU au repos | ~0 % (100 % événementiel, aucun polling actif) |
| Démarrage à froid | < 200 ms |
| Binaire | < 15 Mo |
| Rendu | uniquement pendant une animation ou un changement d'état |

C'est la raison principale pour **ne pas** partir sur une webview (Tauri/WebView2 ≈ 80–150 Mo de RAM).

---

## 2. Choix techniques

### UI : Slint (recommandé)

- Rust natif, très léger (renderer femtovg / skia / logiciel).
- Langage déclaratif `.slint` **interprétable au runtime** (`slint-interpreter`) → les utilisateurs peuvent écrire leurs propres layouts sans recompiler. C'est le cœur de la customisation.
- Fenêtres sans bordure + fond transparent supportés.

Alternatives envisagées : `iced` (plus de code pour les animations, pas de layouts au runtime), `egui` (immediate-mode = redessine en continu, mauvais pour le 0 % CPU), Tauri (customisation CSS géniale mais trop lourd).

### Intégration Win32 (crate `windows`)

- Fenêtre `WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE` : pas dans la taskbar, ne vole jamais le focus.
- Zone cliquable réduite à la pilule (hit-test personnalisé), le reste laisse passer les clics.
- Coins arrondis / acrylique via DWM (`DwmSetWindowAttribute`).
- **Masquage auto en plein écran** (jeux, vidéos, présentations) via `SHQueryUserNotificationState`.
- Multi-écrans + DPI par moniteur (`Per-Monitor V2`).
- Icône de tray (`tray-icon`) : réglages, pause, quitter.
- Démarrage avec Windows (clé `Run` ou tâche planifiée).

### Architecture du workspace

```
boringwindows/
├─ crates/
│  ├─ bw-app/        # binaire principal : fenêtre, tray, boucle d'événements
│  ├─ bw-core/       # bus d'événements, état global, trait Module
│  ├─ bw-config/     # TOML + hot reload (notify), validation, thèmes
│  ├─ bw-ui/         # composants Slint, moteur de layout/thème
│  ├─ bw-hook/       # mini binaire relais pour les hooks Claude Code
│  └─ modules/
│     ├─ bw-claude/
│     ├─ bw-media/
│     └─ bw-calendar/
└─ themes/           # thèmes et layouts .slint fournis par défaut
```

Chaque module implémente un trait commun :

```rust
pub trait Module: Send + 'static {
    fn id(&self) -> &'static str;
    /// Démarre les tâches async du module ; publie des événements sur le bus.
    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()>;
    /// Actions venant de l'UI (clic sur "next", "allow", ...).
    fn on_action(&mut self, action: Action);
    /// Priorité pour décider qui prend la pilule en mode compact.
    fn attention(&self) -> Attention; // None | Low | High | Urgent
}
```

- Runtime : `tokio` (single-thread suffit) ; communication module → UI via channels, l'UI ne fait qu'afficher un état.
- **Arbitrage d'attention** : en mode compact une seule chose s'affiche. Ordre par défaut : Claude qui attend > réunion imminente > musique > rien. Configurable.
- Modules activables/désactivables : un module désactivé n'est même pas démarré (coût zéro).

---

## 3. Modules

### 3.1 Claude Code — le différenciateur (MVP)

Même principe que coucou, éprouvé :

1. **Relais `bw-hook.exe`** (binaire minuscule, démarrage instantané) installé dans `%LOCALAPPDATA%\BoringWindows\bin\`.
2. Il est déclaré dans `%USERPROFILE%\.claude\settings.json` pour les événements :
   - `SessionStart` / `SessionEnd` → suivre les sessions actives ;
   - `UserPromptSubmit` / `PreToolUse` → « Claude travaille… » ;
   - `Notification` → **Claude a besoin de toi** ;
   - `PermissionRequest` → demande d'autorisation, **répondable depuis l'île** ;
   - `Stop` → « Terminé ✓ ».
3. Le relais lit le JSON sur stdin et l'envoie à l'app via **named pipe** `\\.\pipe\boringwindows`.
4. **Timeout strict (~300 ms)** : si l'app est fermée/lente, le hook sort proprement et Claude Code continue normalement. On ne bloque *jamais* une session.
5. Pour `PermissionRequest`, l'app peut renvoyer `allow`/`deny` dans le pipe → le relais l'écrit sur stdout. Sans réponse à temps → Claude retombe sur le prompt terminal.

Détails importants :

- **Installation transparente** : bouton dans les réglages qui montre le diff exact de `settings.json`, fait une sauvegarde datée, et ne retire que nos entrées à la désinstallation.
- **Multi-sessions** : suivi par `session_id` + `cwd` → afficher le nom du projet, un badge par session.
- **Clic sur la notif** → ramène au premier plan le terminal concerné (Windows Terminal / VS Code), via le PID parent remonté par le relais.
- **WSL** : un Claude Code lancé dans WSL peut appeler `bw-hook.exe` via l'interop Windows (`/mnt/c/...`). À tester tôt ; fallback TCP `127.0.0.1` si besoin.
- Bonus : son / vibration visuelle configurable quand Claude attend depuis plus de N secondes.

### 3.2 Musique — via GSMTC (pas besoin d'API)

Windows expose `GlobalSystemMediaTransportControlsSessionManager` (les contrôles média du système). Avec ça, **sans aucune clé d'API** :

- titre, artiste, album, **pochette**, position/durée ;
- **précédent / pause / suivant**, seek ;
- compatible Spotify desktop, **Apple Music (app Windows)**, navigateurs (YouTube, Deezer, SoundCloud…), VLC, etc.

Donc Apple Music n'est pas mort : il remonte via GSMTC comme les autres.

- 100 % événementiel (`MediaPropertiesChanged`, `PlaybackInfoChanged`) → 0 % CPU.
- Couleur d'accent extraite de la pochette (petit k-means sur une miniature) pour teinter l'île.
- Visualiseur audio optionnel (WASAPI loopback) — désactivé par défaut car c'est le seul truc qui consomme en continu.
- Choix de la source si plusieurs lecteurs actifs.

**Spotify Web API (phase ultérieure, optionnelle)** : seulement pour ce que GSMTC ne donne pas — liker, file d'attente, playlists, contrôler un autre appareil (Spotify Connect). OAuth PKCE. Attention : les apps Spotify en « development mode » sont limitées (quota d'utilisateurs, compte Premium requis), donc ça doit rester un bonus, jamais une dépendance.

### 3.3 Calendrier

Approche par paliers, du plus simple au plus intégré :

1. **URL ICS** (lecture seule) : marche avec Google, Outlook, iCloud, Proton… zéro OAuth. Parfait pour la v1.
2. **Google Calendar API** — OAuth 2.0 PKCE avec redirection loopback (`127.0.0.1:<port>`).
3. **Microsoft Graph** (Outlook / Microsoft 365) — même flux OAuth.
4. **CalDAV** — iCloud (mot de passe d'application), Fastmail, Nextcloud…

Fonctionnalités :

- prochain événement + compte à rebours dans la pilule ;
- alerte « réunion dans 5 min » qui prend l'attention ;
- **bouton « Rejoindre »** si un lien Meet / Teams / Zoom est détecté ;
- vue agenda du jour en mode étendu.

Sécurité : tokens stockés dans le **Gestionnaire d'identifiants Windows** (crate `keyring`), jamais en clair dans la config. Synchro toutes les ~5 min + au réveil du PC.

### 3.4 Idées de modules suivants

| Module | Notes |
|---|---|
| Shelf de fichiers | glisser-déposer des fichiers sur l'île pour les garder sous la main (comme boring.notch) |
| Volume / luminosité | OSD qui remplace celui de Windows |
| Batterie / charge | animation au branchement |
| Minuteur / Pomodoro | simple et très demandé |
| Presse-papiers | derniers éléments copiés |
| Notifications Windows | `UserNotificationListener` exige une identité de package (MSIX) → à garder pour plus tard |
| Stats système | CPU/RAM/GPU, à la demande uniquement |

---

## 4. Customisation

Trois niveaux, du plus simple au plus puissant :

### Niveau 1 — `config.toml` (`%APPDATA%\BoringWindows\config.toml`)

```toml
[general]
monitor = "primary"        # "primary" | "cursor" | "all"
hide_in_fullscreen = true
open_on = "hover"          # "hover" | "click"

[theme]
name = "default"
accent = "auto"            # "auto" = couleur de la pochette
corner_radius = 18
blur = "acrylic"           # "none" | "acrylic" | "mica"

[layout]
compact  = ["claude", "media", "calendar"]   # ordre de priorité
expanded = [["media", "calendar"], ["claude"]]

[modules.claude]
enabled = true
sound_after_secs = 30

[modules.media]
enabled = true
visualizer = false

[[modules.calendar.sources]]
kind = "ics"
url = "https://…/basic.ics"
```

Rechargement à chaud : on sauvegarde, l'île se met à jour instantanément.

### Niveau 2 — Thèmes

Un dossier `themes/<nom>/` avec couleurs, polices, durées/courbes d'animation. Partageables (un zip, ou plus tard une galerie).

### Niveau 3 — Layouts `.slint`

Remplacer la vue d'un module par son propre fichier `.slint`, chargé par l'interpréteur au runtime. Les données exposées (titre, pochette, statut Claude…) sont documentées comme une API stable.

### Plus tard — plugins

Modules tiers en **WASM** (via `extism` ou `wasmtime`), sandboxés, qui publient des données que l'UI affiche. À ne faire qu'une fois l'API interne stabilisée.

---

## 5. Roadmap

### Phase 0 — Fondations
- [ ] Workspace Cargo + CI GitHub Actions `windows-latest` (build, clippy, fmt, tests)
- [ ] Fenêtre île : sans bordure, transparente, topmost, no-activate, centrée en haut
- [ ] États compact / étendu + animations (morph de taille)
- [ ] Tray icon, démarrage avec Windows, instance unique
- [ ] `bw-config` : TOML + hot reload
- [ ] Masquage en plein écran, multi-écrans, DPI
- [ ] Bus d'événements + trait `Module` + arbitrage d'attention
- [ ] Mesure perfs dès le début (RAM/CPU suivis en CI ou script)

### Phase 1 — Claude Code (MVP publiable)
- [ ] `bw-hook.exe` + named pipe + timeout 300 ms
- [ ] Installation / désinstallation des hooks avec diff + backup
- [ ] Statuts : travaille / attend / terminé, multi-sessions
- [ ] Répondre à `PermissionRequest` depuis l'île
- [ ] Clic → focus du terminal
- [ ] Test WSL

➡️ **Release v0.1** : déjà utile seule, c'est l'accroche du projet.

### Phase 2 — Musique
- [ ] GSMTC : métadonnées, pochette, prev / play-pause / next, seek
- [ ] Accent couleur depuis la pochette
- [ ] Sélection de la source
- [ ] (opt.) visualiseur WASAPI

➡️ **v0.2**

### Phase 3 — Calendrier
- [ ] Sources ICS
- [ ] Prochain événement, alerte avant réunion, bouton « Rejoindre »
- [ ] OAuth Google + Microsoft Graph, tokens dans le Credential Manager
- [ ] CalDAV

➡️ **v0.3**

### Phase 4 — Customisation avancée
- [ ] Système de thèmes + 2–3 thèmes fournis
- [ ] Layouts `.slint` au runtime + doc de l'API de données
- [ ] UI de réglages (fenêtre Slint séparée, chargée à la demande)

### Phase 5 — Extensions
- [ ] Spotify Web API (like, queue, Connect)
- [ ] Shelf de fichiers, volume/luminosité, minuteur
- [ ] Plugins WASM

### Phase 6 — Distribution
- [ ] Installeur (MSI via `cargo-wix` ou Inno Setup) + package **winget**
- [ ] Signature de code (ex. SignPath, gratuit pour l'open source) pour éviter SmartScreen
- [ ] Mise à jour automatique (GitHub Releases)
- [ ] (opt.) MSIX pour débloquer les API à identité de package (notifications)

---

## 6. Risques & points à valider tôt

| Risque | Mitigation |
|---|---|
| Transparence / topmost / no-activate avec Slint + winit | **Prototype jetable en Phase 0** avant tout le reste ; fallback : fenêtre Win32 native + renderer Slint |
| Animations fluides sans brûler le CPU | ne redessiner que pendant les transitions ; vérifier le 0 % au repos |
| Hooks Claude qui changent de format | parsing tolérant (`serde` avec champs optionnels), tests sur JSON réels |
| Quotas / politique Spotify | GSMTC couvre 95 % du besoin, Spotify reste optionnel |
| SmartScreen sur un binaire non signé | signature dès la première release publique |
| Dev depuis Linux | tout le code Win32 derrière `cfg(windows)`, logique métier testable partout, CI Windows obligatoire |

---

## 7. Prochaine étape concrète

1. Initialiser le workspace Cargo et la CI Windows.
2. Prototype Phase 0 : la pilule transparente qui s'agrandit au survol, en mesurant RAM/CPU.
3. Si le prototype tient les objectifs → enchaîner sur `bw-hook` + named pipe.
