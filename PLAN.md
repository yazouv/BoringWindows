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

### Phase 0 — Fondations ✅
- [x] Workspace Cargo (la CI GitHub Actions a été retirée : trop de minutes consommées ; vérifs en local)
- [x] Fenêtre île : sans bordure, transparente, topmost, no-activate, centrée en haut
- [x] États compact / attention / ouvert + animations (morph de taille)
- [x] Zone cliquable limitée à la pilule (`SetWindowRgn`), le reste laisse passer les clics
- [x] Tray icon, démarrage avec Windows, instance unique
- [x] `bw-config` : TOML + hot reload, erreurs de config affichées dans l'île
- [x] Masquage en plein écran (notification appbar `ABN_FULLSCREENAPP`, sans polling), multi-écrans, DPI
- [x] Bus d'événements + trait `Module` + arbitrage d'attention (+ module `demo`)
- [x] Mesure perfs : `scripts/measure-idle.ps1`, exécuté en CI à titre indicatif

Validé sur Windows : transparence, clics qui traversent, focus jamais volé,
masquage en plein écran. Reste à relever les chiffres RAM/CPU sur un vrai GPU.

### Phase 1 — Claude Code ✅ (à valider sur Windows avec un vrai Claude Code)
- [x] Relais `boringwindows hook` (copié en `bw-hook.exe`) + named pipe / socket Unix + délai de 300 ms
- [x] Installation / désinstallation des hooks : confirmation, sauvegarde datée, seules nos entrées touchées
- [x] Statuts : travaille / attend / terminé, multi-sessions, la plus urgente dans la pilule
- [x] Répondre à `PermissionRequest` depuis l'île (Autoriser / Refuser / Terminal), repli sur le terminal après délai
- [x] Clic sur une session → focus du terminal (console du relais, sinon processus parents)
- [ ] Test WSL (Claude Code dans WSL qui appelle `bw-hook.exe` par l'interop)
- [x] Son / rappel si Claude attend depuis longtemps (`modules.claude.remind_secs`, 30 s par défaut, 0 = désactivé)

Vérifié sous Linux (Xvfb) de bout en bout : événements reçus, clic sur
« Autoriser » → le relais renvoie `behavior: allow` à Claude ; ~6 ms par
événement, ~7 ms quand l'app est fermée.

➡️ **Release v0.1** : déjà utile seule, c'est l'accroche du projet.

### Phase 2 — Musique ✅ (à valider sur Windows avec de vrais lecteurs)
- [x] GSMTC : titre, artiste, pochette, précédent / lecture-pause / suivant, seek par clic sur la barre
- [x] Accent couleur depuis la pochette (teinte dominante, éclaircie pour le fond noir)
- [x] Sélection de la source : celle qui joue d'abord, `⇄` pour changer, liste `ignore`
- [x] 100 % événementiel ; la progression ne tourne (1 Hz) que quand l'île est ouverte et que ça joue
- [ ] (opt.) visualiseur WASAPI

Vérifié sous Linux avec le lecteur du module `demo` (mêmes données que GSMTC) :
pause, suivant, seek, changement de pochette et d'accent, pilule avec pochette.

➡️ **v0.2**

### Phase 3 — Calendrier (ICS ✅, comptes à venir)
- [x] Sources ICS (Google, Outlook/M365, iCloud, Proton…), `webcal://`, fichiers locaux
- [x] Récurrences (RRULE), exceptions (EXDATE, RECURRENCE-ID), annulations, fuseaux IANA et Windows
- [x] Prochain événement, compte à rebours, rappel avant réunion, bouton « Rejoindre » (Teams, Meet, Zoom, Webex…)
- [x] Téléchargement via le client HTTP de Windows (proxy et certificats système), réveils calés sur les échéances
- [ ] OAuth Google + Microsoft Graph, tokens dans le Credential Manager (demande d'enregistrer une app chez chacun)
- [x] CalDAV (iCloud avec mot de passe d'application, Fastmail, Nextcloud) : découverte des agendas, REPORT sur la période, assistant dans les réglages, doc FR + EN
- [x] Liens ICS et mots de passe CalDAV dans le Credential Manager (`secret:<id>` dans config.toml, crate `bw-secrets`)

Vérifié sous Linux : calendrier servi en HTTP, réunion dans 3 min avec lien
Teams replié sur deux lignes, récurrence Outlook, journée entière, clic sur
« Rejoindre » qui ouvre le bon lien.

➡️ **v0.3**

### Phase 3.5 — Réglages sans toucher au TOML

Objectif : qu'on n'ait jamais besoin d'ouvrir `config.toml`. Il reste la source
de vérité (versionnable, partageable), mais une interface l'édite pour nous.

**Fenêtre de réglages** (clic droit sur l'icône › « Réglages… ») ✅
- [x] Fenêtre Slint (style Fluent), `boringwindows --settings` pour l'ouvrir au lancement
- [x] Écriture `toml_edit` (`bw-config::ConfigEditor`) : commentaires conservés,
      validation complète (modules compris) avant d'écrire, écriture atomique
- [x] Général, Apparence (accent avec pastilles, fond, arrondi, animations),
      Agenda (assistant 6 services, guide détaillé, Parcourir…, Tester, nom par défaut),
      Claude Code (hooks, son, délai, diagnostic dans la fenêtre), Musique
- [x] Musique : lecteurs vus récemment à cocher (liste des lecteurs lus depuis le lancement, plus ceux déjà ignorés)
- [ ] Captures d'écran par service dans l'assistant
- [x] Tailles de l'île (compacte / ouverte) et ordre des modules (priorité en mode compact)

- Fenêtre Slint séparée, créée à l'ouverture et détruite à la fermeture : coût
  nul en RAM le reste du temps.
- Écriture via `toml_edit` : les commentaires et l'ordre du fichier sont
  conservés ; le rechargement à chaud applique tout immédiatement.
- Onglets :
  - **Général** : écran, ouverture au survol/clic, masquage en plein écran,
    lancement au démarrage.
  - **Apparence** : couleurs (sélecteur), tailles, coins, animations, avec
    aperçu en direct sur l'île.
  - **Agenda** : liste des calendriers + assistant « Ajouter un calendrier » :
    1. choisir : Google, Outlook / Microsoft 365, iCloud, Proton, autre lien,
       fichier `.ics` ;
    2. instructions pas à pas propres au service (où trouver le lien ICS),
       avec captures ;
    3. coller le lien (ou parcourir un fichier) → bouton **Tester** qui
       télécharge et affiche « 42 événements, prochain : … » avant d'enregistrer.
  - **Claude Code** : installer / retirer les hooks, son, délai de réponse,
    bouton « Diagnostic » (le `doctor` actuel, affiché dans la fenêtre).
  - **Musique** : couleur de pochette, sources ignorées (liste des lecteurs
    vus récemment, à cocher).
- Plus tard : connexion Google / Microsoft (OAuth) directement depuis
  l'assistant, pour les agendas pro dont la publication ICS est interdite.

**Langues de l'app** ✅ français + anglais
- [x] `general.language = "auto" | "fr" | "en"`, réglable dans la fenêtre (onglet Général), appliqué à chaud
- [x] Slint : textes source en anglais `@tr(...)`, français dans `crates/bw-app/lang/fr/LC_MESSAGES/bw-app.po`
      (embarqué), test qui refuse un texte sans traduction
- [x] Rust : crate `bw-i18n`, macro `tr!("anglais", "français")` (agenda, Claude, diagnostic, menu, erreurs de config)
- [ ] Commentaires du `config.toml` par défaut (restent en français)
- [ ] Journal du relais et logs (volontairement en français : destinés au débogage)

Prévu à l'origine :
- Réglage `general.language = "auto" | "fr" | "en"` (auto = langue de Windows).
- Textes de l'UI Slint marqués `@tr(...)`, traductions embarquées dans le
  binaire (`slint-build` + fichiers `.po`) : changement de langue à chaud.
- Textes côté Rust (statuts Claude, agenda, menus de l'icône, messages)
  regroupés dans un petit catalogue `bw-i18n` au lieu d'être écrits en dur.
- La doc et l'app partagent les mêmes termes (glossaire fr/en dans `docs/`).

**Documentation en ligne (GitHub Pages)** ✅ français + anglais (`docs/fr`, `docs/en`), publiée depuis `main`
- Site statique généré depuis `docs/` (mdBook ou page simple) et publié par une
  GitHub Action à chaque push sur la branche principale.
- Pages : installation, premier lancement, un guide par source d'agenda
  (captures à l'appui), Claude Code (installation des hooks, diagnostic),
  référence complète de `config.toml`, FAQ / dépannage.
- L'assistant de la fenêtre de réglages renvoie vers la page du service
  choisi.

Ordre conseillé : la doc d'abord (rapide, utile tout de suite, et elle sert
de texte aux écrans de l'assistant), puis la fenêtre de réglages.

### Phase 4 — Customisation avancée
- [x] Thèmes : `theme.name`, 4 fournis (default, light, midnight, glass), thèmes perso
      `themes/<nom>.toml` rechargés à chaud, clés de `[theme]` prioritaires, contour et police,
      choix dans Réglages › Apparence, page de doc FR + EN
- [x] Layouts `.slint` au runtime (`layout.view`, slint-interpreter + ComponentContainer, rechargement à chaud) + doc de l'API de données, FR + EN

### Phase 5 — Extensions
- [ ] Spotify Web API (like, queue, Connect)
- [x] Minuteur (module `bw-timer`, durées prédéfinies, doc FR + EN)
- [x] Étagère de fichiers (glisser-déposer sur l'île, `[modules.shelf]`, doc FR + EN)
- [x] Volume (module `bw-volume`, callback WASAPI sans polling, doc FR + EN)
- [ ] Luminosité (WMI, écrans intégrés seulement : non testable sur un PC fixe)
- [ ] Plugins WASM

### Port macOS (en parallèle, sans bloquer Windows)

Le cœur (`bw-core`, `bw-config`, l'UI Slint) est déjà multiplateforme ; seul
`platform/` est spécifique. Aujourd'hui sur macOS : compilation + binaire en CI,
icône de barre de menus, pas d'icône dans le Dock. L'île s'ouvre comme une
fenêtre flottante, sans placement ni zone cliquable.

- [ ] `platform/macos.rs` : `NSPanel` non activant, niveau au-dessus de la barre de menus, centré sous l'encoche
- [ ] Zone cliquable (`ignoresMouseEvents` selon la position) et masquage en plein écran
- [ ] Bundle `.app` signé + notarisé (sinon Gatekeeper bloque), démarrage à la connexion (`SMAppService`)
- [ ] Modules : musique via MediaRemote / AppleScript, hook Claude via socket Unix au lieu du named pipe

Note : sur Mac, boring.notch existe déjà ; l'intérêt est surtout d'avoir la même
île et la même config sur les deux machines.

### Phase 6 — Distribution
- [x] release-please : PR de release (version + CHANGELOG) ; la fusionner publie la release
- [x] Binaires Windows / macOS / Linux + `.sha256` attachés à chaque release (`release.yml`)
- [x] CI des PR sur un seul runner Linux (clippy des 3 OS, tests)
- [x] Mise à jour automatique (GitHub Releases, empreinte vérifiée, jeton facultatif pour un dépôt privé)
- [x] Installeur Inno Setup (job `installer` de la release, par utilisateur) + manifestes **winget** (première soumission à la main, puis `wingetcreate` avec `WINGET_TOKEN`) : voir `packaging/README.md`
- [ ] Signature de code (ex. SignPath, gratuit pour l'open source) pour éviter SmartScreen
- [ ] macOS : bundle `.app` signé + binaire Intel / universel
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
