//! Orchestration sur le thread UI : config, modules, forme de l'île, système.

use std::cell::{Cell, OnceCell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::Duration;

use bw_claude::install::Installer;
use bw_claude::{ClaudeConfig, ClaudeModule, SessionKind, Snapshot};
use bw_config::{Config, ConfigError, ConfigWatcher, OpenOn};
use bw_core::{Action, Arbiter, Attention, Module, ModuleEvent, ModuleEventKind, ModuleHost};
use bw_media::{MediaConfig, MediaModule, MediaSnapshot};
use slint::winit_030::WinitWindowAccessor;
use slint::{
    ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode, VecModel,
};

use crate::geometry::{self, Shape};
use crate::platform::{self, Platform, PlatformEvent, Tray, TrayCommand};
use crate::{ClaudePrompt, ClaudeRow, Island, MediaInfo, clock, demo};

/// Pseudo-module utilisé pour signaler une config invalide dans l'île.
const CONFIG_ERROR: &str = "config";
/// Pseudo-module des messages brefs de l'app (« hooks installés »…).
const FLASH: &str = "app";
/// Sessions Claude affichées dans l'île ouverte (moins quand le lecteur est là).
const MAX_CLAUDE_ROWS: usize = 3;
const MAX_CLAUDE_ROWS_WITH_MEDIA: usize = 2;

thread_local! {
    static CONTROLLER: OnceCell<Rc<Controller>> = const { OnceCell::new() };
}

/// Exécute `f` sur le thread UI, depuis n'importe quel thread.
fn post(f: impl FnOnce(&Rc<Controller>) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || {
        CONTROLLER.with(|c| {
            if let Some(c) = c.get() {
                f(c);
            }
        });
    });
}

pub fn run() -> anyhow::Result<()> {
    let path = bw_config::config_path();
    let (config, config_error) = match Config::load_or_create(&path) {
        Ok(config) => (config, None),
        Err(e) => (Config::default(), Some(e)),
    };

    let controller = Rc::new(Controller {
        ui: Island::new()?,
        arbiter: RefCell::new(Arbiter::new(config.layout.compact.clone())),
        config: RefCell::new(config),
        path,
        platform: RefCell::new(None),
        tray: RefCell::new(None),
        watcher: RefCell::new(None),
        host: RefCell::new(None),
        module_ids: RefCell::new(Vec::new()),
        shape: Cell::new(Shape::Compact),
        expanded: Cell::new(false),
        paused: Cell::new(false),
        fullscreen: Cell::new(false),
        collapse_timer: Timer::default(),
        region_timer: Timer::default(),
        clock_timer: Timer::default(),
        flash_timer: Timer::default(),
        claude: RefCell::new(None),
        installer: Installer::default(),
        media: RefCell::new(None),
        artwork: RefCell::new(None),
        progress_timer: Timer::default(),
    });
    CONTROLLER.with(|c| {
        let _ = c.set(controller.clone());
    });

    controller.start(config_error)?;
    slint::run_event_loop_until_quit()?;
    controller.shutdown();
    Ok(())
}

pub struct Controller {
    ui: Island,
    path: PathBuf,
    config: RefCell<Config>,
    arbiter: RefCell<Arbiter>,
    platform: RefCell<Option<Platform>>,
    tray: RefCell<Option<Tray>>,
    watcher: RefCell<Option<ConfigWatcher>>,
    host: RefCell<Option<ModuleHost>>,
    module_ids: RefCell<Vec<&'static str>>,
    shape: Cell<Shape>,
    expanded: Cell<bool>,
    paused: Cell<bool>,
    fullscreen: Cell<bool>,
    collapse_timer: Timer,
    region_timer: Timer,
    clock_timer: Timer,
    flash_timer: Timer,
    /// Dernier état Claude reçu (pour retrouver le terminal d'une session).
    claude: RefCell<Option<Arc<Snapshot>>>,
    installer: Installer,
    /// Dernier état musical et module qui l'a publié (destinataire des actions).
    media: RefCell<Option<(&'static str, Arc<MediaSnapshot>)>>,
    /// Pochette convertie pour Slint, gardée tant que le morceau ne change pas.
    artwork: RefCell<Option<(Arc<Vec<u8>>, Image)>>,
    progress_timer: Timer,
}

impl Controller {
    fn start(self: &Rc<Self>, config_error: Option<ConfigError>) -> anyhow::Result<()> {
        self.apply_theme();
        let (choice, size) = self.placement();
        if let Some(pos) = platform::initial_position(choice, size) {
            self.ui.window().set_position(pos);
        }
        self.ui.show()?;

        // La fenêtre native n'est créée qu'une fois la boucle d'événements
        // lancée : l'intégration système se branche à ce moment-là.
        let this = self.clone();
        slint::spawn_local(async move {
            let attached = match this.ui.window().winit_window().await {
                Ok(window) => Platform::attach(&window, |event| {
                    post(move |c| c.on_platform_event(event));
                }),
                Err(e) => Err(e.into()),
            };
            match attached {
                Ok(platform) => this.on_platform_ready(platform),
                Err(e) => {
                    log::error!("intégration système impossible : {e:#}");
                    let _ = slint::quit_event_loop();
                }
            }
        })?;

        let weak = Rc::downgrade(self);
        self.ui
            .on_hover_changed(with(&weak, |c, hovered| c.on_hover(hovered)));
        let on_click = with(&weak, |c, ()| c.set_expanded(!c.expanded.get()));
        self.ui.on_clicked(move || on_click(()));
        self.ui.on_claude_decide({
            let weak = weak.clone();
            move |id, decision| {
                if let Some(c) = weak.upgrade() {
                    c.on_claude_decide(&id, &decision);
                }
            }
        });
        self.ui
            .on_claude_focus(with(&weak, |c, session: slint::SharedString| {
                c.focus_claude_session(&session);
            }));
        self.ui
            .on_media_action(with(&weak, |c, action: slint::SharedString| {
                c.media_action(action.to_string());
            }));
        self.ui
            .on_media_seek(with(&weak, |c, fraction: f32| c.media_seek(fraction)));
        self.update_clock();

        match bw_config::watch(&self.path, |res| post(move |c| c.on_config(res))) {
            Ok(w) => *self.watcher.borrow_mut() = Some(w),
            Err(e) => log::warn!("rechargement à chaud désactivé : {e}"),
        }

        self.restart_modules();
        if let Some(e) = config_error {
            self.report_config_error(&e);
        }
        Ok(())
    }

    fn on_platform_ready(&self, platform: Platform) {
        log::info!("intégration système prête");
        self.fullscreen.set(platform.fullscreen_now());
        *self.platform.borrow_mut() = Some(platform);
        self.update_visibility();
        self.sync_region();

        // Créée boucle d'événements lancée : exigé par macOS.
        let hooks_installed = self.installer.is_installed();
        match Tray::new(platform::autostart_enabled(), hooks_installed, |cmd| {
            post(move |c| c.on_tray(cmd));
        }) {
            Ok(tray) => *self.tray.borrow_mut() = Some(tray),
            Err(e) => log::error!("icône de notification indisponible : {e:#}"),
        }

        // Relais à jour après une recompilation ou une mise à jour de l'app.
        if hooks_installed && let Ok(exe) = std::env::current_exe() {
            match self.installer.refresh_binary(&exe, false) {
                Ok(true) => log::info!("relais Claude mis à jour"),
                Ok(false) => {}
                Err(e) => log::warn!("relais Claude non mis à jour : {e:#}"),
            }
        }
    }

    fn shutdown(&self) {
        if let Some(host) = self.host.borrow_mut().take() {
            host.shutdown();
        }
        self.watcher.borrow_mut().take();
        self.tray.borrow_mut().take();
        self.platform.borrow_mut().take();
    }

    // --- Forme de l'île ----------------------------------------------------

    fn set_expanded(&self, expanded: bool) {
        self.collapse_timer.stop();
        if self.expanded.replace(expanded) != expanded {
            self.refresh_shape();
            self.update_progress();
        }
    }

    fn on_hover(self: &Rc<Self>, hovered: bool) {
        let general = self.config.borrow().general.clone();
        if hovered {
            self.collapse_timer.stop();
            if general.open_on == OpenOn::Hover {
                self.set_expanded(true);
            }
        } else if self.expanded.get() {
            let weak = Rc::downgrade(self);
            self.collapse_timer.start(
                TimerMode::SingleShot,
                Duration::from_millis(general.collapse_delay_ms.into()),
                move || {
                    if let Some(c) = weak.upgrade() {
                        c.set_expanded(false);
                    }
                },
            );
        }
    }

    /// Recalcule l'état affiché après un changement (ouverture, attention…).
    fn refresh_shape(&self) {
        let winner = self.arbiter.borrow().winner();
        let new = if self.expanded.get() {
            Shape::Expanded
        } else if winner.is_some() {
            Shape::Attention
        } else {
            Shape::Compact
        };

        self.ui.set_expanded(self.expanded.get());
        self.ui.set_has_attention(winner.is_some());
        self.ui.set_urgent(
            winner
                .as_ref()
                .is_some_and(|w| w.level == Attention::Urgent),
        );
        // Musique en tête de pilule : la pochette remplace le point.
        let media_wins = winner.as_ref().is_some_and(|w| {
            self.media
                .borrow()
                .as_ref()
                .is_some_and(|(owner, _)| w.module == *owner)
        });
        let art = self.artwork.borrow().as_ref().map(|(_, img)| img.clone());
        self.ui.set_compact_art_visible(media_wins && art.is_some());
        if let Some(art) = art.filter(|_| media_wins) {
            self.ui.set_compact_art(art);
        }
        self.ui
            .set_attention_label(winner.and_then(|w| w.summary).unwrap_or_default().into());

        let old = self.shape.replace(new);
        if old == new {
            return;
        }

        // Pendant l'animation, la zone cliquable couvre l'ancienne et la
        // nouvelle forme ; elle est ajustée à la fin.
        let config = self.config.borrow();
        let scale = self.ui.window().scale_factor();
        let during = geometry::pill_rect(&config.theme, old, scale).union(geometry::pill_rect(
            &config.theme,
            new,
            scale,
        ));
        self.with_platform(|p| p.set_hit_region(during));
        self.schedule_region_sync(Duration::from_millis(config.theme.animation_ms.into()));
    }

    fn schedule_region_sync(&self, delay: Duration) {
        self.region_timer.start(
            TimerMode::SingleShot,
            delay + Duration::from_millis(30),
            || post(|c| c.sync_region()),
        );
    }

    fn sync_region(&self) {
        let rect = geometry::pill_rect(
            &self.config.borrow().theme,
            self.shape.get(),
            self.ui.window().scale_factor(),
        );
        self.with_platform(|p| p.set_hit_region(rect));
    }

    fn update_visibility(&self) {
        let hide_fullscreen = self.config.borrow().general.hide_in_fullscreen;
        let visible = !self.paused.get() && !(self.fullscreen.get() && hide_fullscreen);
        self.with_platform(|p| p.set_visible(visible));
    }

    fn with_platform(&self, f: impl FnOnce(&Platform)) {
        if let Some(p) = self.platform.borrow().as_ref() {
            f(p);
        }
    }

    fn placement(&self) -> (bw_config::MonitorChoice, (f32, f32)) {
        let config = self.config.borrow();
        (config.general.monitor, geometry::window_size(&config.theme))
    }

    fn place(&self) {
        let (choice, size) = self.placement();
        self.with_platform(|p| p.place(self.ui.window(), choice, size));
    }

    fn update_clock(&self) {
        let (text, next) = clock::now();
        self.ui.set_time_text(text.time.into());
        self.ui.set_date_text(text.date.into());
        self.clock_timer
            .start(TimerMode::SingleShot, next, || post(|c| c.update_clock()));
    }

    // --- Config ------------------------------------------------------------

    fn apply_theme(&self) {
        let config = self.config.borrow();
        let t = &config.theme;
        let ui = &self.ui;
        ui.set_compact_width(t.compact.width);
        ui.set_compact_height(t.compact.height);
        ui.set_attention_width(t.attention.width);
        ui.set_attention_height(t.attention.height);
        ui.set_expanded_width(t.expanded.width);
        ui.set_expanded_height(t.expanded.height);
        ui.set_corner_radius(t.corner_radius);
        ui.set_top_offset(t.top_offset);
        ui.set_anim(t.animation_ms.into());
        ui.set_bg(color(t.background));
        ui.set_fg(color(t.foreground));
        drop(config);
        self.apply_accent();
    }

    fn on_config(self: &Rc<Self>, result: Result<Config, ConfigError>) {
        let new = match result {
            Ok(new) => new,
            Err(e) => return self.report_config_error(&e),
        };
        log::info!("config rechargée");

        let old = self.config.replace(new);
        let config = self.config.borrow();
        self.arbiter
            .borrow_mut()
            .claim(CONFIG_ERROR, Attention::None, None);
        self.arbiter
            .borrow_mut()
            .set_priority(config.layout.compact.clone());
        let moved = old.general.monitor != config.general.monitor
            || geometry::window_size(&old.theme) != geometry::window_size(&config.theme);
        drop(config);

        self.apply_theme();
        if moved {
            self.place();
        }
        // Forcer la mise à jour de la zone cliquable (tailles changées).
        self.refresh_shape();
        self.sync_region();
        self.update_visibility();
        if old.modules != self.config.borrow().modules {
            self.restart_modules();
        }
    }

    fn report_config_error(&self, e: &ConfigError) {
        log::error!("{e}");
        let first_line = e.to_string().lines().next().unwrap_or_default().to_owned();
        self.arbiter.borrow_mut().claim(
            CONFIG_ERROR,
            Attention::High,
            Some(format!("⚠ {first_line}")),
        );
        self.refresh_shape();
    }

    // --- Modules -----------------------------------------------------------

    fn restart_modules(&self) {
        if let Some(host) = self.host.borrow_mut().take() {
            host.shutdown();
        }
        {
            let mut arbiter = self.arbiter.borrow_mut();
            for id in self.module_ids.borrow().iter() {
                arbiter.claim(id, Attention::None, None);
            }
        }

        self.apply_claude(None);
        *self.media.borrow_mut() = None;
        *self.artwork.borrow_mut() = None;
        self.ui.set_has_media(false);
        self.progress_timer.stop();
        let (modules, errors) = build_modules(&self.config.borrow());
        for e in errors {
            log::error!("{e}");
            self.arbiter
                .borrow_mut()
                .claim(CONFIG_ERROR, Attention::High, Some(format!("⚠ {e}")));
        }
        *self.module_ids.borrow_mut() = modules.iter().map(|m| m.id()).collect();
        log::info!("modules actifs : {:?}", self.module_ids.borrow());

        match ModuleHost::spawn(modules, |event| post(move |c| c.on_module_event(event))) {
            Ok(host) => *self.host.borrow_mut() = Some(host),
            Err(e) => log::error!("impossible de démarrer les modules : {e:#}"),
        }
        self.refresh_shape();
    }

    fn on_module_event(&self, event: ModuleEvent) {
        // Événement tardif d'un module arrêté entre-temps.
        if !self.module_ids.borrow().contains(&event.module) {
            return;
        }
        match event.kind {
            ModuleEventKind::Attention { level, summary } => {
                if self
                    .arbiter
                    .borrow_mut()
                    .claim(event.module, level, summary)
                {
                    self.refresh_shape();
                }
            }
            ModuleEventKind::State(state) => {
                if let Some(snapshot) = state.downcast_ref::<MediaSnapshot>() {
                    self.apply_media(event.module, Arc::new(snapshot.clone()));
                } else if event.module == bw_claude::MODULE_ID
                    && let Ok(snapshot) = state.downcast::<Snapshot>()
                {
                    self.apply_claude(Some(snapshot));
                }
            }
        }
    }

    // --- Musique ------------------------------------------------------------

    fn apply_media(&self, owner: &'static str, snapshot: Arc<MediaSnapshot>) {
        let np = snapshot.now_playing.as_ref();

        // Pochette : conversion seulement quand elle change.
        let art = np.and_then(|n| n.artwork.as_ref());
        let image = art.map(|a| {
            let mut cache = self.artwork.borrow_mut();
            match cache.as_ref() {
                Some((pixels, img)) if Arc::ptr_eq(pixels, &a.rgba) => img.clone(),
                _ => {
                    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                        &a.rgba, a.width, a.height,
                    );
                    let img = Image::from_rgba8(buffer);
                    *cache = Some((a.rgba.clone(), img.clone()));
                    img
                }
            }
        });
        if image.is_none() {
            *self.artwork.borrow_mut() = None;
        }

        self.ui.set_has_media(np.is_some());
        if let Some(n) = np {
            self.ui.set_media(MediaInfo {
                title: n.title.as_str().into(),
                artist: n.artist.as_str().into(),
                source: n.source.as_str().into(),
                playing: n.playing,
                can_previous: n.can_previous,
                can_next: n.can_next,
                can_toggle: n.can_play_pause,
                can_seek: n.can_seek && n.duration.is_some(),
                has_art: image.is_some(),
                art: image.unwrap_or_default(),
                has_timeline: n.duration.is_some(),
                position: "".into(),
                duration: n.duration.map(format_time).unwrap_or_default().into(),
                progress: 0.0,
                multi_source: snapshot.source_count > 1,
            });
        }

        *self.media.borrow_mut() = np.is_some().then_some((owner, snapshot));
        self.apply_accent();
        self.update_progress();
        self.refresh_shape();
    }

    /// Teinte de l'île : couleur de la pochette en cours, sinon celle du thème.
    fn apply_accent(&self) {
        let from_artwork = self
            .media
            .borrow()
            .as_ref()
            .filter(|_| self.media_config().accent_from_artwork)
            .and_then(|(_, s)| s.now_playing.as_ref()?.artwork.as_ref()?.accent);
        let accent = from_artwork.map_or_else(
            || color(self.config.borrow().theme.accent),
            |[r, g, b]| slint::Color::from_rgb_u8(r, g, b),
        );
        self.ui.set_accent(accent);
    }

    fn media_config(&self) -> MediaConfig {
        MediaConfig::from_table(self.config.borrow().modules.get(bw_media::MODULE_ID))
            .unwrap_or_default()
    }

    /// Met à jour la barre de progression ; tourne à 1 Hz seulement quand
    /// l'île est ouverte et que la musique joue.
    fn update_progress(&self) {
        let media = self.media.borrow();
        let Some(np) = media.as_ref().and_then(|(_, s)| s.now_playing.as_ref()) else {
            self.progress_timer.stop();
            return;
        };
        let position = np.position_now(std::time::Instant::now());
        let mut info = self.ui.get_media();
        info.position = format_time(position).into();
        info.progress = np
            .duration
            .map_or(0.0, |d| position.as_secs_f32() / d.as_secs_f32().max(1.0));
        self.ui.set_media(info);

        let ticking = self.expanded.get() && np.playing && np.duration.is_some();
        if ticking && !self.progress_timer.running() {
            self.progress_timer
                .start(TimerMode::Repeated, Duration::from_secs(1), || {
                    post(|c| c.update_progress())
                });
        } else if !ticking {
            self.progress_timer.stop();
        }
    }

    fn media_action(&self, action: String) {
        let owner = self.media.borrow().as_ref().map(|(owner, _)| *owner);
        if let (Some(owner), Some(host)) = (owner, self.host.borrow().as_ref()) {
            host.send_action(Action {
                module: owner.into(),
                name: action,
            });
        }
    }

    fn media_seek(&self, fraction: f32) {
        let duration = self
            .media
            .borrow()
            .as_ref()
            .and_then(|(_, s)| s.now_playing.as_ref()?.duration);
        if let Some(d) = duration {
            let ms = (d.as_millis() as f64 * f64::from(fraction.clamp(0.0, 1.0))) as u64;
            self.media_action(format!("seek:{ms}"));
        }
    }

    // --- Claude Code -------------------------------------------------------

    fn apply_claude(&self, snapshot: Option<Arc<Snapshot>>) {
        let max_rows = if self.ui.get_has_media() {
            MAX_CLAUDE_ROWS_WITH_MEDIA
        } else {
            MAX_CLAUDE_ROWS
        };
        let rows: Vec<ClaudeRow> = snapshot.as_ref().map_or_else(Vec::new, |s| {
            let rank = |k: SessionKind| match k {
                SessionKind::Permission | SessionKind::NeedsYou => 0,
                SessionKind::Done | SessionKind::Working => 1,
                SessionKind::Idle => 2,
            };
            let mut sessions: Vec<_> = s.sessions.iter().collect();
            sessions.sort_by_key(|s| rank(s.kind));
            sessions
                .into_iter()
                .take(max_rows)
                .map(|s| ClaudeRow {
                    id: s.id.as_str().into(),
                    project: s.project.as_str().into(),
                    status: s.status.as_str().into(),
                    urgent: rank(s.kind) == 0,
                    active: s.kind != SessionKind::Idle,
                })
                .collect()
        });
        self.ui.set_claude_rows(ModelRc::new(VecModel::from(rows)));

        let prompt = snapshot.as_ref().and_then(|s| s.prompt.as_ref());
        self.ui.set_has_prompt(prompt.is_some());
        if let Some(p) = prompt {
            self.ui.set_prompt(ClaudePrompt {
                id: p.id.to_string().into(),
                project: p.project.as_str().into(),
                tool: p.tool.as_str().into(),
                detail: p.detail.as_str().into(),
            });
        }
        *self.claude.borrow_mut() = snapshot;
    }

    fn on_claude_decide(&self, id: &str, decision: &str) {
        if let Some(host) = self.host.borrow().as_ref() {
            host.send_action(Action {
                module: bw_claude::MODULE_ID.into(),
                name: format!("{decision}:{id}"),
            });
        }
        // « Terminal » : la question passe dans le terminal, on l'y amène.
        if decision == "ask" {
            let session = self
                .claude
                .borrow()
                .as_ref()
                .and_then(|s| s.prompt.as_ref())
                .filter(|p| p.id.to_string() == id)
                .map(|p| p.session_id.clone());
            if let Some(session) = session {
                self.focus_claude_session(&session);
            }
        }
    }

    fn focus_claude_session(&self, session: &str) {
        let target = self.claude.borrow().as_ref().and_then(|s| {
            s.sessions
                .iter()
                .find(|v| v.id == session)
                .map(|v| (v.ancestors.clone(), v.console_window))
        });
        if let Some((ancestors, console)) = target
            && !platform::focus_terminal(&ancestors, console)
        {
            log::info!("terminal de la session {session} introuvable");
        }
    }

    fn toggle_claude_hooks(self: &Rc<Self>) {
        let installer = &self.installer;
        let installed = installer.is_installed();
        let settings = installer.settings_path.display();
        let question = if installed {
            format!(
                "Retirer les hooks BoringWindows de {settings} ?\n\nUne sauvegarde du fichier est faite avant modification."
            )
        } else {
            format!(
                "BoringWindows va ajouter ses hooks à {settings} pour les événements :\n{}\n\nTes autres réglages et hooks ne sont pas modifiés, et une sauvegarde du fichier est faite avant.\nLe relais est copié dans {}.",
                bw_claude::install::HOOK_EVENTS.join(", "),
                installer.binary_path.display()
            )
        };
        if !platform::confirm("BoringWindows · Claude Code", &question) {
            return;
        }

        let result = if installed {
            installer.uninstall()
        } else {
            std::env::current_exe()
                .map_err(anyhow::Error::from)
                .and_then(|exe| installer.install(&exe))
        };
        match result {
            Ok(report) => {
                log::info!(
                    "hooks Claude {} ({}, sauvegarde : {:?})",
                    if installed { "retirés" } else { "installés" },
                    report.settings_path.display(),
                    report.backup
                );
                self.flash(if installed {
                    "Hooks Claude Code retirés"
                } else {
                    "Hooks Claude Code installés"
                });
            }
            Err(e) => {
                log::error!("hooks Claude : {e:#}");
                self.flash("⚠ Échec des hooks Claude (voir les logs)");
            }
        }
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.set_claude_hooks_installed(installer.is_installed());
        }
    }

    /// Message bref dans la pilule.
    fn flash(self: &Rc<Self>, text: &str) {
        self.arbiter
            .borrow_mut()
            .claim(FLASH, Attention::High, Some(text.to_owned()));
        self.refresh_shape();
        let weak = Rc::downgrade(self);
        self.flash_timer
            .start(TimerMode::SingleShot, Duration::from_secs(4), move || {
                if let Some(c) = weak.upgrade() {
                    c.arbiter.borrow_mut().claim(FLASH, Attention::None, None);
                    c.refresh_shape();
                }
            });
    }

    // --- Système -----------------------------------------------------------

    fn on_platform_event(&self, event: PlatformEvent) {
        match event {
            PlatformEvent::Fullscreen(entering) => {
                let relevant = self
                    .platform
                    .borrow()
                    .as_ref()
                    .is_some_and(Platform::foreground_on_our_monitor);
                self.fullscreen.set(entering && relevant);
                self.update_visibility();
            }
            PlatformEvent::DisplayChanged => {
                self.place();
                // Laisse le temps au changement de DPI d'être appliqué.
                self.schedule_region_sync(Duration::from_millis(250));
            }
        }
    }

    fn on_tray(self: &Rc<Self>, command: TrayCommand) {
        match command {
            TrayCommand::OpenConfig => platform::open_path(&self.path),
            TrayCommand::ReloadConfig => self.on_config(Config::load_or_create(&self.path)),
            TrayCommand::Autostart(enabled) => {
                if let Err(e) = platform::set_autostart(enabled) {
                    log::error!("démarrage automatique : {e:#}");
                    if let Some(tray) = self.tray.borrow().as_ref() {
                        tray.set_autostart_checked(!enabled);
                    }
                }
            }
            TrayCommand::Pause(paused) => {
                self.paused.set(paused);
                self.update_visibility();
            }
            TrayCommand::ClaudeHooks => self.toggle_claude_hooks(),
            TrayCommand::Quit => {
                let _ = slint::quit_event_loop();
            }
        }
    }
}

/// Modules actifs selon la config, et erreurs de config propres aux modules.
fn build_modules(config: &Config) -> (Vec<Box<dyn Module>>, Vec<String>) {
    let mut modules: Vec<Box<dyn Module>> = Vec::new();
    let mut errors = Vec::new();
    if config.module_enabled(bw_claude::MODULE_ID, true) {
        match ClaudeConfig::from_table(config.modules.get(bw_claude::MODULE_ID)) {
            Ok(c) => modules.push(Box::new(ClaudeModule::new(c, bw_claude::ipc::endpoint()))),
            Err(e) => errors.push(format!("{e:#}")),
        }
    }
    if MediaModule::is_supported() && config.module_enabled(bw_media::MODULE_ID, true) {
        match MediaConfig::from_table(config.modules.get(bw_media::MODULE_ID)) {
            Ok(c) => modules.push(Box::new(MediaModule::new(c))),
            Err(e) => errors.push(format!("modules.media : {e:#}")),
        }
    }
    if config.module_enabled("demo", false) {
        modules.push(Box::new(demo::Demo::default()));
    }
    (modules, errors)
}

/// « 3:07 », « 1:02:45 ».
fn format_time(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

fn color(c: bw_config::Color) -> slint::Color {
    slint::Color::from_argb_u8(c.a, c.r, c.g, c.b)
}

/// Adapte une méthode du contrôleur en callback Slint sans cycle de références.
fn with<A>(
    weak: &Weak<Controller>,
    f: impl Fn(&Rc<Controller>, A) + 'static,
) -> impl Fn(A) + 'static {
    let weak = weak.clone();
    move |arg| {
        if let Some(c) = weak.upgrade() {
            f(&c, arg);
        }
    }
}
