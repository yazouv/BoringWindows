//! Orchestration sur le thread UI : config, modules, forme de l'île, système.

mod custom_view;
mod settings;
mod update;

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::Duration;

use bw_calendar::{CalendarConfig, CalendarModule, CalendarSnapshot};
use bw_claude::install::Installer;
use bw_claude::{ClaudeConfig, ClaudeModule, SessionKind, Snapshot};
use bw_config::{Config, ConfigError, ConfigWatcher, OpenOn};
use bw_core::{Action, Arbiter, Attention, Module, ModuleEvent, ModuleEventKind, ModuleHost};
use bw_i18n::tr;
use bw_media::{MediaConfig, MediaModule, MediaSnapshot};
use bw_notify::{NotifyConfig, NotifyModule, NotifySnapshot};
use bw_timer::{Phase as TimerPhase, TimerConfig, TimerModule, TimerSnapshot};
use slint::winit_030::WinitWindowAccessor;
use slint::{
    ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode, VecModel,
};

use crate::geometry::{self, Shape};
use crate::gestures::{self, Gesture, GesturesConfig};
use crate::mascot::{self, ClaudeState, MascotConfig};
use crate::platform::{self, Platform, PlatformEvent, Tray, TrayCommand};
use crate::shelf::{Shelf, ShelfConfig};
use crate::{
    AgendaRow, ClaudePrompt, ClaudeRow, GaugeInfo, Island, MediaInfo, NotifInfo, PluginRow,
    RecentRow, ShelfRow, TimerInfo, WeatherInfo, clock, demo,
};

/// Pseudo-module utilisé pour signaler une config invalide dans l'île.
const CONFIG_ERROR: &str = "config";
/// Pseudo-module des messages brefs de l'app (« hooks installés »…).
const FLASH: &str = "app";
/// Lignes (agenda + sessions Claude) qui tiennent dans l'île ouverte, avec ou
/// sans la carte du lecteur.
const MAX_ROWS: usize = 4;
const MAX_ROWS_WITH_MEDIA: usize = 2;
/// Notifications qui tiennent dans leur onglet.
const NOTIF_ROWS: usize = 4;
/// Onglets de l'île ouverte (propriété `tab` de `island.slint`).
const TAB_NOTIFICATIONS: i32 = 2;
/// Avec le flou : cadence de suivi de la pilule pendant une animation, et
/// marge après sa fin.
const BLUR_FRAME: Duration = Duration::from_millis(16);
const BLUR_TAIL: Duration = Duration::from_millis(80);

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

/// `open_settings` : ouvrir la fenêtre de réglages dès le démarrage (`--settings`).
pub fn run(open_settings: bool) -> anyhow::Result<()> {
    // Avant de lire la config : le thème « auto » dépend du mode de Windows.
    let look = platform::system_look();
    log::info!(
        "apparence de Windows : {} ; accent {:?}",
        if look.light { "claire" } else { "sombre" },
        look.accent
    );
    bw_config::set_system_light(look.light);
    let path = bw_config::config_path();
    let (config, config_error) = match Config::load_or_create(&path) {
        Ok(config) => (config, None),
        Err(e) => (Config::default(), Some(e)),
    };
    bw_i18n::set(language(config.general.language));
    let shelf_file = path.with_file_name("shelf.txt");
    // Présent : « ne pas déranger » activé (gardé d'un lancement à l'autre).
    let dnd_file = path.with_file_name("do-not-disturb");

    let controller = Rc::new(Controller {
        // Seule l'île reçoit les attributs de fenêtre spéciaux (pas de focus,
        // transparente…) ; les autres fenêtres restent normales.
        ui: platform::creating_island(Island::new)?,
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
        calendar: RefCell::new(None),
        settings: RefCell::new(None),
        open_settings_at_start: Cell::new(open_settings),
        update_state: Cell::new(update::UpdateState::Idle),
        update_timer: Timer::default(),
        artwork: RefCell::new(None),
        progress_timer: Timer::default(),
        remind_timer: Timer::default(),
        timer: RefCell::new(None),
        custom_view: RefCell::new(None),
        shelf: RefCell::new(Shelf::load(&shelf_file)),
        shelf_file,
        viz_active: Cell::new(false),
        activity: RefCell::new(None),
        timer_tick: Timer::default(),
        media_seen: RefCell::new(BTreeSet::new()),
        notify: RefCell::new(None),
        notif_seq: Cell::new(0),
        taskbar: Cell::new(None),
        dnd: Cell::new(dnd_file.exists()),
        dnd_file,
        app_icons: RefCell::new(std::collections::HashMap::new()),
        look: Cell::new(look),
        look_watcher: RefCell::new(None),
        blur: Cell::new(false),
        blur_timer: Timer::default(),
        blur_until: Cell::new(None),
        gauges: RefCell::new(std::collections::HashMap::new()),
        away: Cell::new(false),
        away_timer: Timer::default(),
        wheel: RefCell::new(gestures::Wheel::default()),
        live: Cell::new(false),
        toast_timer: Timer::default(),
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
    remind_timer: Timer,
    /// Dernier état du minuteur (absent si le module est désactivé).
    timer: RefCell<Option<Arc<TimerSnapshot>>>,
    custom_view: RefCell<Option<custom_view::CustomView>>,
    shelf: RefCell<Shelf>,
    shelf_file: PathBuf,
    viz_active: Cell<bool>,
    activity: RefCell<Option<Arc<bw_claude::ActivitySnapshot>>>,
    timer_tick: Timer,
    /// Lecteurs vus depuis le lancement (jetons `ignore`), pour les réglages.
    media_seen: RefCell<BTreeSet<String>>,
    calendar: RefCell<Option<Arc<CalendarSnapshot>>>,
    /// Dernier état des notifications (absent si le module est désactivé).
    notify: RefCell<Option<Arc<NotifySnapshot>>>,
    /// Compteur passé à l'UI pour rejouer l'animation d'arrivée.
    notif_seq: Cell<i32>,
    /// Hauteur (logique) de la barre des tâches quand elle est en haut de
    /// l'écran de l'île.
    taskbar: Cell<Option<f32>>,
    /// Ne pas déranger : les notifications ne s'annoncent plus.
    dnd: Cell<bool>,
    dnd_file: PathBuf,
    /// Icônes des applications converties pour Slint, par AppUserModelID.
    app_icons: RefCell<std::collections::HashMap<String, Image>>,
    settings: RefCell<Option<settings::SettingsState>>,
    open_settings_at_start: Cell<bool>,
    update_state: Cell<update::UpdateState>,
    update_timer: Timer,
    /// Mode clair/sombre et accent de Windows, et leur abonnement.
    look: Cell<platform::SystemLook>,
    look_watcher: RefCell<Option<platform::LookWatcher>>,
    /// Flou actif : la région de la fenêtre suit la pilule image par image
    /// pendant les animations (`blur_timer`) jusqu'à `blur_until`.
    blur: Cell<bool>,
    blur_timer: Timer,
    blur_until: Cell<Option<std::time::Instant>>,
    /// Dernière jauge publiée par chaque module (batterie, Bluetooth).
    gauges: RefCell<std::collections::HashMap<&'static str, bw_power::Gauge>>,
    /// Personne au clavier depuis `sleep_after_minutes` : la mascotte dort.
    away: Cell<bool>,
    away_timer: Timer,
    /// Cumul des défilements (gestes).
    wheel: RefCell<gestures::Wheel>,
    /// En appel ou en partage d'écran (`bw-presence`).
    live: Cell<bool>,
    toast_timer: Timer,
}

impl Controller {
    fn start(self: &Rc<Self>, config_error: Option<ConfigError>) -> anyhow::Result<()> {
        select_ui_language();
        self.apply_theme();
        self.reload_custom_view();
        self.update_shelf_ui();
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
        self.ui
            .on_recent_open(with(&weak, |c, id: slint::SharedString| c.open_recent(&id)));
        self.ui
            .on_notif_action(with(&weak, |c, action: slint::SharedString| {
                c.notif_action(&action);
            }));
        self.ui.on_dnd_toggle({
            let weak = weak.clone();
            move || {
                if let Some(c) = weak.upgrade() {
                    c.set_dnd(!c.dnd.get());
                }
            }
        });
        self.ui.set_dnd(self.dnd.get());
        self.ui
            .on_shelf_open(with(&weak, |c, i: i32| c.shelf_open(i as usize)));
        self.ui
            .on_shelf_remove(with(&weak, |c, i: i32| c.shelf_remove(i as usize)));
        self.ui.window().on_winit_window_event(|_, event| {
            use slint::winit_030::winit::event::WindowEvent;
            match event {
                WindowEvent::DroppedFile(path) => {
                    log::info!("étagère : fichier déposé {}", path.display());
                    let path = path.clone();
                    post(move |c| c.shelf_dropped(vec![path]));
                }
                WindowEvent::HoveredFile(path) => {
                    log::info!("étagère : survol de {}", path.display());
                    post(|c| {
                        if c.shelf_config().enabled {
                            c.collapse_timer.stop();
                            c.set_expanded(true);
                        }
                    });
                }
                WindowEvent::HoveredFileCancelled => post(|c| c.on_hover(false)),
                _ => {}
            }
            slint::winit_030::EventResult::Propagate
        });
        self.ui.on_wheel({
            let weak = weak.clone();
            move |dx, dy, shift| {
                if let Some(c) = weak.upgrade() {
                    c.on_wheel(dx, dy, shift);
                }
            }
        });
        self.ui.on_drag(with(&weak, |c, dx: f32| c.on_drag(dx)));
        self.ui.on_long_press({
            let weak = weak.clone();
            move || {
                if let Some(c) = weak.upgrade() {
                    c.on_long_press();
                }
            }
        });
        self.ui
            .on_timer_action(with(&weak, |c, action: slint::SharedString| {
                c.timer_action(action.to_string());
            }));
        self.ui.on_open_url(|url| {
            // Uniquement des liens web : jamais de chemin ou de commande venus d'un ICS.
            if url.starts_with("https://") {
                log::info!("ouverture : {url}");
                platform::open_path(std::path::Path::new(url.as_str()));
            }
        });
        self.update_clock();

        match bw_config::watch(&self.path, |res| post(move |c| c.on_config(res))) {
            Ok(w) => *self.watcher.borrow_mut() = Some(w),
            Err(e) => log::warn!("rechargement à chaud désactivé : {e}"),
        }
        *self.look_watcher.borrow_mut() =
            platform::watch_system_look(|| post(|c| c.on_system_look()));

        self.restart_modules();
        if let Some(e) = config_error {
            self.report_config_error(&e);
        }
        Ok(())
    }

    fn on_platform_ready(self: &Rc<Self>, platform: Platform) {
        log::info!("intégration système prête");
        if self.open_settings_at_start.get() {
            self.open_settings();
        }
        self.fullscreen.set(platform.fullscreen_now());
        *self.platform.borrow_mut() = Some(platform);
        self.update_taskbar();
        self.update_visibility();
        self.apply_blur();
        self.apply_capture_exclusion();
        self.schedule_away();

        // Créée boucle d'événements lancée : exigé par macOS.
        let hooks_installed = self.installer.is_installed();
        match Tray::new(
            platform::autostart_enabled(),
            hooks_installed,
            &self.update_tray_label(),
            |cmd| {
                post(move |c| c.on_tray(cmd));
            },
        ) {
            Ok(tray) => *self.tray.borrow_mut() = Some(tray),
            Err(e) => log::error!("icône de notification indisponible : {e:#}"),
        }
        self.start_updates();

        if !hooks_installed && self.module_ids.borrow().contains(&bw_claude::MODULE_ID) {
            self.flash(&tr!(
                "Claude Code: hooks not installed · right-click the icon",
                "Claude Code : hooks non installés · clic droit sur l'icône"
            ));
        }

        // Hooks d'une version précédente : on complète l'installation.
        if hooks_installed
            && self.installer.needs_upgrade()
            && let Ok(exe) = std::env::current_exe()
        {
            match self.installer.install(&exe) {
                Ok(report) => {
                    log::info!("hooks Claude mis à jour (sauvegarde : {:?})", report.backup);
                    self.flash(&tr!(
                        "Claude hooks updated · restart your Claude sessions",
                        "Hooks Claude mis à jour · relance tes sessions Claude"
                    ));
                }
                Err(e) => log::warn!("mise à jour des hooks Claude : {e:#}"),
            }
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
            if expanded && self.announcing().is_some() {
                self.ui.set_tab(TAB_NOTIFICATIONS);
                self.send_notify("hide");
            }
            self.refresh_shape();
            if expanded {
                self.refresh_activity();
            }
            self.update_progress();
            self.update_viz_activity();
            self.update_timer_ui();
            self.sync_custom_view();
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
        // Une notification ne dure que quelques secondes : elle passe devant
        // tout, sauf une action requise (permission de Claude…).
        let notif_wins = self.announcing().is_some()
            && !self.dnd.get()
            // En appel ou en partage d'écran : rien ne s'annonce.
            && !self.live.get()
            && winner.as_ref().is_some_and(|w| w.level < Attention::Urgent);
        let new = if self.expanded.get() {
            Shape::Expanded
        } else if notif_wins {
            Shape::Notification
        } else if winner.is_some() {
            Shape::Attention
        } else {
            Shape::Compact
        };

        self.ui.set_expanded(self.expanded.get());
        self.ui.set_has_attention(winner.is_some());
        self.ui.set_has_notif(notif_wins);
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
        // Batterie, appareil Bluetooth : icône et jauge à la place du point
        // (une icône vide retire la jauge, pour le module demo).
        let gauge = winner
            .as_ref()
            .and_then(|w| self.gauges.borrow().get(w.module.as_str()).cloned())
            .filter(|g| !g.icon.is_empty());
        self.ui.set_compact_gauge_visible(gauge.is_some());
        if let Some(g) = &gauge {
            self.ui.set_compact_gauge(GaugeInfo {
                icon: g.icon.into(),
                level: g.level.map_or(-1, i32::from),
                charging: g.charging,
                low: g.low,
            });
        }
        // La jauge passe devant la pochette (module demo, qui fait les deux).
        let art = self
            .artwork
            .borrow()
            .as_ref()
            .map(|(_, img)| img.clone())
            .filter(|_| media_wins && gauge.is_none());
        self.ui.set_compact_art_visible(art.is_some());
        if let Some(art) = art {
            self.ui.set_compact_art(art);
        }
        self.ui
            .set_attention_label(winner.and_then(|w| w.summary).unwrap_or_default().into());
        self.update_mascot();

        let old = self.shape.replace(new);
        if old == new {
            return;
        }

        let theme = self.theme();
        let animation = Duration::from_millis(theme.animation_ms.into());
        if self.blur.get() {
            // Le flou remplit toute la région : elle suit la pilule pendant
            // l'animation, sinon il déborderait de la forme dessinée.
            self.blur_until
                .set(Some(std::time::Instant::now() + animation + BLUR_TAIL));
            self.blur_timer
                .start(TimerMode::Repeated, BLUR_FRAME, || post(|c| c.blur_frame()));
            self.schedule_region_sync(animation + BLUR_TAIL);
            return;
        }
        // Pendant l'animation, la zone cliquable couvre l'ancienne et la
        // nouvelle forme ; elle est ajustée à la fin.
        let scale = self.ui.window().scale_factor();
        let during =
            geometry::pill_rect(&theme, old, scale).union(geometry::pill_rect(&theme, new, scale));
        self.with_platform(|p| p.set_hit_region(during));
        self.schedule_region_sync(animation);
    }

    fn schedule_region_sync(&self, delay: Duration) {
        self.region_timer.start(
            TimerMode::SingleShot,
            delay + Duration::from_millis(30),
            || post(|c| c.sync_region()),
        );
    }

    fn sync_region(&self) {
        if self.blur.get() {
            return self.sync_round_region();
        }
        let rect = geometry::pill_rect(
            &self.theme(),
            self.shape.get(),
            self.ui.window().scale_factor(),
        );
        self.with_platform(|p| p.set_hit_region(rect));
    }

    /// Région à la forme exacte de la pilule, telle qu'elle est dessinée.
    fn sync_round_region(&self) {
        let ui = &self.ui;
        let shape = geometry::pill_round_rect(
            (
                ui.get_pill_x(),
                ui.get_pill_y(),
                ui.get_pill_width(),
                ui.get_pill_height(),
            ),
            ui.get_pill_radius(),
            ui.get_pill_flat_top(),
            ui.window().scale_factor(),
        );
        self.with_platform(|p| p.set_round_region(shape));
    }

    fn blur_frame(&self) {
        self.sync_round_region();
        let done = self
            .blur_until
            .get()
            .is_none_or(|until| std::time::Instant::now() >= until);
        if done {
            self.blur_timer.stop();
            self.blur_until.set(None);
        }
    }

    /// Flou du thème (`theme.blur`), appliqué à la fenêtre.
    fn apply_blur(&self) {
        let on = self.config.borrow().theme.blur;
        if self.blur.replace(on) != on || on {
            self.with_platform(|p| p.set_blur(on));
        }
        self.blur_timer.stop();
        self.sync_region();
    }

    /// Windows a changé de mode (clair/sombre) ou de couleur d'accent.
    fn on_system_look(self: &Rc<Self>) {
        let look = platform::system_look();
        if self.look.replace(look) == look {
            return;
        }
        log::info!(
            "apparence de Windows : {} ; accent {:?}",
            if look.light { "claire" } else { "sombre" },
            look.accent
        );
        let auto = self.config.borrow().theme.name == bw_config::AUTO_THEME;
        if bw_config::set_system_light(look.light) && auto {
            // Le thème « auto » se résout à la lecture de la config.
            self.on_config(Config::load_or_create(&self.path));
        } else {
            self.apply_accent();
        }
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
        self.update_mascot();
        self.sync_custom_view();
        self.clock_timer
            .start(TimerMode::SingleShot, next, || post(|c| c.update_clock()));
    }

    // --- Config ------------------------------------------------------------

    /// Thème de la config, pilules ramenées à la barre des tâches si elle est
    /// en haut et plus basse qu'elles.
    fn theme(&self) -> bw_config::Theme {
        geometry::fit_under_taskbar(&self.config.borrow().theme, self.taskbar.get())
    }

    /// Relit la barre des tâches (position, taille) et redessine si besoin.
    fn update_taskbar(&self) {
        let height = self
            .platform
            .borrow()
            .as_ref()
            .and_then(Platform::top_taskbar_height);
        if self.taskbar.replace(height) != height {
            log::info!("barre des tâches en haut : {height:?}");
            self.apply_theme();
            self.sync_region();
        }
    }

    fn apply_theme(&self) {
        let t = &self.theme();
        let ui = &self.ui;
        ui.set_compact_width(t.compact.width);
        ui.set_compact_height(t.compact.height);
        ui.set_attention_width(t.attention.width);
        ui.set_attention_height(t.attention.height);
        ui.set_expanded_width(t.expanded.width);
        ui.set_expanded_height(t.expanded.height);
        let notif = geometry::notification_size(t);
        ui.set_notif_width(notif.width);
        ui.set_notif_height(notif.height);
        ui.set_corner_radius(t.corner_radius);
        ui.set_top_offset(t.top_offset);
        ui.set_anim(t.animation_ms.into());
        ui.set_bg(color(t.background));
        ui.set_fg(color(t.foreground));
        ui.set_border(color(t.border));
        ui.set_font(t.font.as_str().into());
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
        let relabel = old.general.language != config.general.language;
        let moved = old.general.monitor != config.general.monitor
            || geometry::window_size(&old.theme) != geometry::window_size(&config.theme);
        drop(config);

        self.apply_theme();
        self.apply_blur();
        self.apply_capture_exclusion();
        self.schedule_away();
        let view_changed = {
            let config = self.config.borrow();
            old.layout.view != config.layout.view
                || old.layout.view_stamp != config.layout.view_stamp
        };
        if view_changed {
            self.reload_custom_view();
        }
        self.update_shelf_ui();
        if moved {
            self.place();
        }
        // Forcer la mise à jour de la zone cliquable (tailles changées).
        self.refresh_shape();
        self.sync_region();
        self.update_visibility();
        if relabel {
            bw_i18n::set(language(self.config.borrow().general.language));
            select_ui_language();
            self.retranslate();
        }
        if relabel || old.modules != self.config.borrow().modules {
            // Les textes des modules (agenda, Claude…) sont refaits au redémarrage.
            self.restart_modules();
        }
    }

    /// Remet dans la langue courante les textes produits côté Rust.
    fn retranslate(self: &Rc<Self>) {
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.retranslate();
        }
        self.sync_update_ui();
        self.update_clock();
        self.retranslate_settings();
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

        *self.calendar.borrow_mut() = None;
        *self.timer.borrow_mut() = None;
        *self.notify.borrow_mut() = None;
        self.ui.set_has_notif_tab(false);
        self.ui.set_notif_rows(ModelRc::default());
        self.ui.set_unread_colors(ModelRc::default());
        self.ui.set_notif_unread(0);
        self.viz_active.set(false);
        *self.activity.borrow_mut() = None;
        self.ui.set_has_claude_tab(false);
        self.ui.set_recent_rows(ModelRc::default());
        self.ui.set_plugin_rows(ModelRc::default());
        self.ui.set_viz_bars(ModelRc::default());
        self.timer_tick.stop();
        self.ui.set_has_timer(false);
        self.ui.set_has_weather(false);
        self.live.set(false);
        self.ui.set_live(false);
        self.gauges.borrow_mut().clear();
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
        if self.dnd.get() {
            self.send_notify("dnd:on");
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
                } else if let Ok(snapshot) = state.clone().downcast::<bw_claude::ActivitySnapshot>()
                {
                    self.apply_activity(snapshot);
                } else if let Some(snapshot) = state.downcast_ref::<bw_plugins::PluginsSnapshot>() {
                    self.apply_plugins(snapshot);
                } else if let Some(snapshot) = state.downcast_ref::<bw_viz::VizSnapshot>() {
                    self.apply_viz(snapshot);
                } else if let Ok(snapshot) = state.clone().downcast::<NotifySnapshot>() {
                    self.apply_notify(snapshot);
                } else if let Some(snapshot) = state.downcast_ref::<TimerSnapshot>() {
                    self.apply_timer(snapshot.clone());
                } else if let Some(live) = state.downcast_ref::<bw_presence::LiveSnapshot>() {
                    self.apply_live(live);
                } else if let Some(gauge) = state.downcast_ref::<bw_power::Gauge>() {
                    // Publiée juste avant l'attention qu'elle accompagne.
                    self.gauges.borrow_mut().insert(event.module, gauge.clone());
                } else if let Some(weather) = state.downcast_ref::<bw_weather::WeatherSnapshot>() {
                    self.ui.set_has_weather(true);
                    self.ui.set_weather(WeatherInfo {
                        icon: weather.sky.icon(weather.day).into(),
                        temperature: weather.temperature_text().into(),
                        detail: weather.detail().into(),
                    });
                } else if let Ok(snapshot) = state.clone().downcast::<CalendarSnapshot>() {
                    *self.calendar.borrow_mut() = Some(snapshot);
                    self.layout_rows();
                } else if event.module == bw_claude::MODULE_ID
                    && let Ok(snapshot) = state.downcast::<Snapshot>()
                {
                    self.apply_claude(Some(snapshot));
                }
            }
        }
    }

    // --- Activité Claude : conversations récentes et consommation ------------

    fn apply_activity(&self, snapshot: Arc<bw_claude::ActivitySnapshot>) {
        let config = bw_claude::ActivityConfig::from_table(
            self.config.borrow().modules.get(bw_claude::ACTIVITY_ID),
        )
        .unwrap_or_default();
        let now = chrono::Utc::now();

        let rows: Vec<RecentRow> = snapshot
            .recent
            .iter()
            .map(|r| RecentRow {
                id: r.id.as_str().into(),
                title: if r.title.is_empty() {
                    tr!("(untitled)", "(sans titre)").into()
                } else {
                    r.title.as_str().into()
                },
                meta: format!("{} · {}", r.project, ago(r.last_at, now)).into(),
            })
            .collect();
        self.ui.set_recent_rows(ModelRc::new(VecModel::from(rows)));

        let usage = &snapshot.usage;
        let limit = snapshot.limit_tokens;
        self.ui.set_usage_text(match usage.window_end {
            Some(end) => {
                let h = config.window_hours;
                let used = compact_tokens(usage.tokens);
                let of = if limit > 0 {
                    format!(" / {}", compact_tokens(limit))
                } else {
                    String::new()
                };
                let reset = duration_text((end - now).to_std().unwrap_or_default());
                tr!(
                    "≈ {used}{of} tokens in the {h} h window · resets in {reset}",
                    "≈ {used}{of} tokens sur la fenêtre de {h} h · reset dans {reset}"
                )
            }
            None => tr!(
                "No active usage window (estimate from local transcripts)",
                "Aucune fenêtre de consommation active (estimation d'après les transcripts locaux)"
            ),
        }
        .into());
        self.ui
            .set_usage_has_limit(limit > 0 && usage.window_end.is_some());
        self.ui.set_usage_ratio(if limit > 0 {
            (usage.tokens as f64 / limit as f64) as f32
        } else {
            0.0
        });
        self.ui.set_has_claude_tab(true);
        *self.activity.borrow_mut() = Some(snapshot);
        self.sync_custom_view();
    }

    /// Rouvre la conversation `id` dans un terminal.
    fn open_recent(&self, id: &str) {
        let session = self
            .activity
            .borrow()
            .as_ref()
            .and_then(|a| a.recent.iter().find(|r| r.id == id).cloned());
        if let Some(session) = session
            && !platform::resume_claude_session(&session.cwd, &session.id)
        {
            self.settings_status(
                &tr!(
                    "Could not reopen the conversation (is `claude` installed?)",
                    "Impossible de rouvrir la conversation (`claude` est-il installé ?)"
                ),
                true,
            );
        }
    }

    fn refresh_activity(&self) {
        if let Some(host) = self.host.borrow().as_ref() {
            host.send_action(Action {
                module: bw_claude::ACTIVITY_ID.into(),
                name: "refresh".into(),
            });
        }
    }

    // --- Notifications des applications --------------------------------------

    fn apply_notify(&self, snapshot: Arc<NotifySnapshot>) {
        let previous = self.announcing().map(|n| n.id);
        if let Some(n) = snapshot.announcing.as_ref()
            && previous != Some(n.id)
        {
            self.ui.set_notif(self.notif_info(n, ""));
            self.notif_seq.set(self.notif_seq.get().wrapping_add(1));
            self.ui.set_notif_seq(self.notif_seq.get());
        }

        self.ui.set_has_notif_tab(true);
        // Arrivée pendant que l'onglet est sous les yeux : déjà lue.
        if snapshot.unread > 0 && self.expanded.get() && self.ui.get_tab() == TAB_NOTIFICATIONS {
            self.send_notify("seen");
        }

        *self.notify.borrow_mut() = Some(snapshot);
        self.update_notif_ui();
        self.refresh_shape();
    }

    /// Liste de l'onglet et points des non lues (masqués en « ne pas déranger »).
    fn update_notif_ui(&self) {
        let notify = self.notify.borrow();
        let Some(snapshot) = notify.as_ref() else {
            return;
        };
        let dnd = self.dnd.get();
        // En « ne pas déranger », une ligne de l'onglet le rappelle.
        let shown = NOTIF_ROWS - usize::from(dnd);
        let now = chrono::Utc::now();
        let rows: Vec<NotifInfo> = snapshot
            .recent
            .iter()
            .take(shown)
            .map(|n| self.notif_info(n, &ago(n.at.into(), now)))
            .collect();
        self.ui.set_notif_rows(ModelRc::new(VecModel::from(rows)));

        // Un point par application, dans l'ordre d'arrivée des non lues.
        let mut colors: Vec<slint::Color> = Vec::new();
        for n in snapshot.recent.iter().take(snapshot.unread) {
            let c = rgb(n.color());
            if !dnd && !colors.contains(&c) && colors.len() < 3 {
                colors.push(c);
            }
        }
        self.ui
            .set_unread_colors(ModelRc::new(VecModel::from(colors)));
        self.ui.set_notif_unread(snapshot.unread as i32);
    }

    /// Active ou coupe « ne pas déranger », et s'en souvient.
    fn set_dnd(&self, on: bool) {
        self.dnd.set(on);
        let saved = if on {
            std::fs::write(&self.dnd_file, "")
        } else {
            std::fs::remove_file(&self.dnd_file).or_else(|e| match e.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(e),
            })
        };
        if let Err(e) = saved {
            log::warn!("ne pas déranger : {} : {e}", self.dnd_file.display());
        }
        log::info!("ne pas déranger : {on}");
        self.ui.set_dnd(on);
        self.send_notify(if on { "dnd:on" } else { "dnd:off" });
        self.update_notif_ui();
        self.refresh_shape();
    }

    /// Notification annoncée dans la pilule en ce moment.
    fn announcing(&self) -> Option<bw_notify::Notification> {
        self.notify.borrow().as_ref()?.announcing.clone()
    }

    fn notif_action(&self, action: &str) {
        let Some(id) = action.strip_prefix("open:") else {
            return self.send_notify(action);
        };
        let Some(n) = self.notify.borrow().as_ref().and_then(|s| {
            s.recent
                .iter()
                .find(|n| (n.id as i32).to_string() == id)
                .cloned()
        }) else {
            return;
        };
        let original =
            NotifyConfig::from_table(self.config.borrow().modules.get(bw_notify::MODULE_ID))
                .unwrap_or_default()
                .open_original;
        // Rejouer la notification bloque un instant (centre de notifications à
        // ouvrir, puis à parcourir) : hors du thread UI.
        let spawned = std::thread::Builder::new()
            .name("bw-notify-open".into())
            .spawn(move || {
                if original && platform::open_notification(&n.lines) {
                    return;
                }
                if !platform::open_app(&n.app_id) {
                    log::info!("application {} introuvable", n.app_id);
                }
            });
        if let Err(e) = spawned {
            log::warn!("notifications : ouverture impossible : {e}");
        }
    }

    fn notif_info(&self, n: &bw_notify::Notification, ago: &str) -> NotifInfo {
        let icon = n.icon.as_ref().map(|icon| {
            self.app_icons
                .borrow_mut()
                .entry(n.app_id.clone())
                .or_insert_with(|| {
                    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                        &icon.rgba,
                        icon.width,
                        icon.height,
                    ))
                })
                .clone()
        });
        NotifInfo {
            id: n.id as i32,
            app: n.app.as_str().into(),
            title: n.title.as_str().into(),
            body: n.body.as_str().into(),
            tint: rgb(n.color()),
            initial: n.initial().into(),
            has_icon: icon.is_some(),
            icon: icon.unwrap_or_default(),
            ago: ago.into(),
        }
    }

    fn send_notify(&self, action: &str) {
        if let Some(host) = self.host.borrow().as_ref() {
            host.send_action(Action {
                module: bw_notify::MODULE_ID.into(),
                name: action.into(),
            });
        }
    }

    // --- Plugins WASM -------------------------------------------------------

    fn apply_plugins(&self, snapshot: &bw_plugins::PluginsSnapshot) {
        // Deux lignes au plus : la place est comptée.
        let rows: Vec<PluginRow> = snapshot
            .items
            .iter()
            .take(2)
            .map(|i| PluginRow {
                name: i.name.as_str().into(),
                text: i.text.as_str().into(),
                attention: i32::from(i.attention),
            })
            .collect();
        self.ui.set_plugin_rows(ModelRc::new(VecModel::from(rows)));
        self.layout_rows();
    }

    // --- Visualiseur --------------------------------------------------------

    /// La capture audio ne tourne que si l'île est ouverte et qu'une musique joue.
    fn update_viz_activity(&self) {
        let present = self.module_ids.borrow().contains(&bw_viz::MODULE_ID);
        let bars = self
            .config
            .borrow()
            .module_enabled(bw_viz::MODULE_ID, false);
        let mascot = self.mascot_config();
        let dance = mascot.enabled && mascot.music;
        let expanded = self.expanded.get();
        // Barres dans l'île ouverte, ou danse de la pilule fermée.
        let active =
            present && self.music_playing() && ((bars && expanded) || (dance && !expanded));
        if active == self.viz_active.replace(active) {
            return;
        }
        if let Some(host) = self.host.borrow().as_ref() {
            host.send_action(Action {
                module: bw_viz::MODULE_ID.into(),
                name: if active { "start" } else { "stop" }.into(),
            });
        }
        if !active {
            self.ui.set_viz_bars(ModelRc::default());
            self.ui.set_mascot_level(0.0);
            self.ui.set_pill_bounce(1.0);
            self.sync_custom_view();
        }
    }

    fn apply_viz(&self, snapshot: &bw_viz::VizSnapshot) {
        if !self.viz_active.get() {
            return;
        }
        if self.expanded.get() {
            // Barres à zéro : elles disparaissent au lieu de rester à plat.
            let flat = snapshot.bands.iter().all(|b| *b <= 0.001);
            let bars = if flat {
                ModelRc::default()
            } else {
                ModelRc::new(VecModel::from(snapshot.bands.clone()))
            };
            self.ui.set_viz_bars(bars);
            self.sync_custom_view();
            return;
        }
        // Pilule fermée : les basses (premières bandes) font danser la
        // mascotte, et la pilule quand elle montre la musique (sans flou :
        // le fond flouté ne suit pas ces petites variations).
        let low = &snapshot.bands[..snapshot.bands.len().min(3)];
        let bass = if low.is_empty() {
            0.0
        } else {
            (low.iter().sum::<f32>() / low.len() as f32).clamp(0.0, 1.0)
        };
        self.ui.set_mascot_level(bass);
        let music_pill = self.ui.get_mascot_place() == "right" && !self.blur.get();
        self.ui.set_pill_bounce(if music_pill { bass } else { 1.0 });
    }

    // --- Étagère ------------------------------------------------------------

    fn shelf_config(&self) -> ShelfConfig {
        ShelfConfig::from_table(self.config.borrow().modules.get("shelf")).unwrap_or_default()
    }

    /// Fichiers déposés sur l'île : on les garde et on montre l'étagère.
    fn shelf_dropped(self: &Rc<Self>, paths: Vec<PathBuf>) {
        let config = self.shelf_config();
        if !config.enabled {
            return;
        }
        {
            let mut shelf = self.shelf.borrow_mut();
            shelf.add(paths, config.max);
            shelf.save(&self.shelf_file);
        }
        self.update_shelf_ui();
        self.collapse_timer.stop();
        self.set_expanded(true);
        self.on_hover(false);
    }

    fn update_shelf_ui(&self) {
        const SHOWN: usize = 4;
        let config = self.shelf_config();
        let items: Vec<PathBuf> = if config.enabled {
            let mut shelf = self.shelf.borrow_mut();
            shelf.truncate(config.max);
            if shelf.prune() {
                shelf.save(&self.shelf_file);
            }
            shelf.items().to_vec()
        } else {
            Vec::new()
        };
        let rows: Vec<ShelfRow> = items
            .iter()
            .take(SHOWN)
            .map(|p| ShelfRow {
                name: crate::shelf::display_name(p).into(),
                path: p.display().to_string().into(),
            })
            .collect();
        self.ui.set_shelf_rows(ModelRc::new(VecModel::from(rows)));
        self.ui.set_shelf_more(
            if items.len() > SHOWN {
                format!("+{}", items.len() - SHOWN)
            } else {
                String::new()
            }
            .into(),
        );
        self.layout_rows();
    }

    fn shelf_open(&self, index: usize) {
        let path = self.shelf.borrow().items().get(index).cloned();
        if let Some(path) = path {
            if path.exists() {
                platform::open_path(&path);
            } else {
                self.update_shelf_ui();
            }
        }
    }

    fn shelf_remove(&self, index: usize) {
        let removed = {
            let mut shelf = self.shelf.borrow_mut();
            let removed = shelf.remove(index).is_some();
            if removed {
                shelf.save(&self.shelf_file);
            }
            removed
        };
        if removed {
            self.update_shelf_ui();
        }
    }

    // --- Minuteur -----------------------------------------------------------

    fn apply_timer(&self, snapshot: TimerSnapshot) {
        let finished = snapshot.phase == TimerPhase::Done
            && self
                .timer
                .borrow()
                .as_ref()
                .is_none_or(|t| t.phase != TimerPhase::Done);
        if finished && self.timer_config().sound {
            platform::alert_sound();
        }
        *self.timer.borrow_mut() = Some(Arc::new(snapshot));
        self.ui.set_has_timer(true);
        self.update_timer_ui();
        self.layout_rows();
    }

    fn timer_config(&self) -> TimerConfig {
        TimerConfig::from_table(self.config.borrow().modules.get(bw_timer::MODULE_ID))
            .unwrap_or_default()
    }

    /// Affiche le minuteur ; les secondes ne défilent que si l'île est ouverte.
    fn update_timer_ui(&self) {
        let timer = self.timer.borrow();
        let Some(t) = timer.as_ref() else {
            self.timer_tick.stop();
            return;
        };
        let now = std::time::Instant::now();
        self.ui.set_timer(TimerInfo {
            phase: match t.phase {
                TimerPhase::Idle => 0,
                TimerPhase::Running => 1,
                TimerPhase::Paused => 2,
                TimerPhase::Done => 3,
            },
            time: format_time(t.remaining_now(now) + Duration::from_millis(999)).into(),
            progress: t.progress_now(now),
            presets: ModelRc::new(VecModel::from(
                t.presets
                    .iter()
                    .map(|m| slint::SharedString::from(m.to_string()))
                    .collect::<Vec<_>>(),
            )),
        });
        let ticking = self.expanded.get() && t.phase == TimerPhase::Running;
        if ticking && !self.timer_tick.running() {
            self.timer_tick
                .start(TimerMode::Repeated, Duration::from_secs(1), || {
                    post(|c| c.update_timer_ui())
                });
        } else if !ticking {
            self.timer_tick.stop();
        }
        drop(timer);
        self.sync_custom_view();
    }

    fn timer_action(&self, action: String) {
        if let Some(host) = self.host.borrow().as_ref() {
            host.send_action(Action {
                module: bw_timer::MODULE_ID.into(),
                name: action,
            });
        }
    }

    // --- Lignes : agenda et sessions Claude ---------------------------------

    /// Répartit les lignes disponibles entre l'agenda et les sessions Claude.
    fn layout_rows(&self) {
        let budget = if self.ui.get_has_media() {
            MAX_ROWS_WITH_MEDIA
        } else {
            MAX_ROWS
        };
        // Les lignes du minuteur et de l'étagère prennent une place chacune,
        // sans jamais vider le reste.
        let extra = usize::from(self.ui.get_has_timer())
            + usize::from(slint::Model::row_count(&self.ui.get_shelf_rows()) > 0)
            + slint::Model::row_count(&self.ui.get_plugin_rows());
        let budget = budget.saturating_sub(extra).max(1);

        let mut agenda: Vec<AgendaRow> = Vec::new();
        if let Some(cal) = self.calendar.borrow().as_ref() {
            agenda.extend(cal.items.iter().map(|i| AgendaRow {
                title: i.title.as_str().into(),
                time: i.time.as_str().into(),
                location: i.location.clone().unwrap_or_default().into(),
                relative: i.relative.clone().unwrap_or_default().into(),
                join_url: i.join_url.clone().unwrap_or_default().into(),
                has_join: i.join_url.is_some(),
                soon: i.soon,
            }));
            if agenda.is_empty()
                && let Some(err) = &cal.error
            {
                agenda.push(AgendaRow {
                    title: err.as_str().into(),
                    time: "⚠".into(),
                    ..AgendaRow::default()
                });
            }
        }

        let mut claude: Vec<ClaudeRow> = Vec::new();
        if let Some(s) = self.claude.borrow().as_ref() {
            let rank = |k: SessionKind| match k {
                SessionKind::Permission | SessionKind::NeedsYou => 0,
                SessionKind::Done | SessionKind::Working => 1,
                SessionKind::Idle => 2,
            };
            let mut sessions: Vec<_> = s.sessions.iter().collect();
            sessions.sort_by_key(|s| rank(s.kind));
            claude.extend(sessions.into_iter().map(|s| ClaudeRow {
                id: s.id.as_str().into(),
                project: s.project.as_str().into(),
                status: s.status.as_str().into(),
                urgent: rank(s.kind) == 0,
                active: s.kind != SessionKind::Idle,
            }));
        }

        let (agenda_rows, claude_rows) = split_rows(agenda.len(), claude.len(), budget);
        agenda.truncate(agenda_rows);
        claude.truncate(claude_rows);
        self.ui
            .set_agenda_rows(ModelRc::new(VecModel::from(agenda)));
        self.ui
            .set_claude_rows(ModelRc::new(VecModel::from(claude)));
        self.sync_custom_view();
    }

    // --- Musique ------------------------------------------------------------

    fn apply_media(&self, owner: &'static str, snapshot: Arc<MediaSnapshot>) {
        let np = snapshot.now_playing.as_ref();
        if let Some(n) = np {
            self.media_seen
                .borrow_mut()
                .insert(bw_media::ignore_token(&n.source_id));
        }

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
        self.layout_rows();
        self.apply_accent();
        self.update_progress();
        self.update_viz_activity();
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
        let system = self
            .look
            .get()
            .accent
            .filter(|_| self.config.borrow().theme.system_accent);
        let accent = from_artwork.or(system).map_or_else(
            || color(self.config.borrow().theme.accent),
            |[r, g, b]| slint::Color::from_rgb_u8(r, g, b),
        );
        self.ui.set_accent(accent);
        self.sync_custom_view();
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
        self.sync_custom_view();
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
        let prompt = snapshot.as_ref().and_then(|s| s.prompt.as_ref());
        // Une session se met à t'attendre (question, plan, permission…) :
        // on prévient au son, Claude ne le fait plus quand nos hooks sont là.
        let waiting = |s: Option<&Snapshot>| -> Vec<(String, String)> {
            s.map(|s| {
                s.sessions
                    .iter()
                    .filter(|v| matches!(v.kind, SessionKind::Permission | SessionKind::NeedsYou))
                    .map(|v| (v.id.clone(), v.status.clone()))
                    .collect()
            })
            .unwrap_or_default()
        };
        let before = waiting(self.claude.borrow().as_deref());
        let now_waiting = waiting(snapshot.as_deref());
        let config = self.claude_config();
        if now_waiting.iter().any(|w| !before.contains(w)) && config.sound {
            platform::alert_sound();
        }
        if now_waiting.is_empty() || !config.sound || config.remind_secs == 0 {
            self.remind_timer.stop();
        } else if !self.remind_timer.running() {
            self.remind_timer.start(
                TimerMode::Repeated,
                Duration::from_secs(config.remind_secs.into()),
                || post(|c| c.remind_claude()),
            );
        }
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
        self.layout_rows();
        self.update_mascot();
    }

    fn remind_claude(&self) {
        let still_waiting = self.claude.borrow().as_ref().is_some_and(|s| {
            s.sessions
                .iter()
                .any(|v| matches!(v.kind, SessionKind::Permission | SessionKind::NeedsYou))
        });
        if still_waiting && self.claude_config().sound {
            platform::alert_sound();
        } else {
            self.remind_timer.stop();
        }
    }

    fn claude_config(&self) -> ClaudeConfig {
        ClaudeConfig::from_table(self.config.borrow().modules.get(bw_claude::MODULE_ID))
            .unwrap_or_default()
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
            tr!(
                "Remove the BoringWindows hooks from {settings}?\n\nA backup of the file is made first.",
                "Retirer les hooks BoringWindows de {settings} ?\n\nUne sauvegarde du fichier est faite avant modification."
            )
        } else {
            tr!(
                "BoringWindows will add its hooks to {settings} for these events:\n{}\n\nYour other settings and hooks are left untouched, and a backup of the file is made first.\nThe relay is copied to {}.",
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
                self.flash(&if installed {
                    tr!("Claude Code hooks removed", "Hooks Claude Code retirés")
                } else {
                    tr!("Claude Code hooks installed", "Hooks Claude Code installés")
                });
            }
            Err(e) => {
                log::error!("hooks Claude : {e:#}");
                self.flash(&tr!(
                    "⚠ Claude hooks failed (see the logs)",
                    "⚠ Échec des hooks Claude (voir les logs)"
                ));
            }
        }
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.set_claude_hooks_installed(installer.is_installed());
        }
    }

    // --- Mascotte ---------------------------------------------------------

    fn mascot_config(&self) -> MascotConfig {
        MascotConfig::from_table(self.config.borrow().modules.get("mascot")).unwrap_or_default()
    }

    fn music_playing(&self) -> bool {
        self.media
            .borrow()
            .as_ref()
            .is_some_and(|(_, s)| s.now_playing.as_ref().is_some_and(|n| n.playing))
    }

    /// Humeur, place et saison de la mascotte d'après l'état du moment.
    fn update_mascot(&self) {
        use chrono::{Datelike, Timelike};

        let config = self.mascot_config();
        let now = chrono::Local::now();
        let season = if config.seasonal {
            mascot::season(now.month(), now.day())
        } else {
            mascot::Season::None
        };
        self.ui.set_season(season.name().into());
        if !config.enabled {
            self.ui.set_mascot_place("".into());
            self.ui.set_mascot_animated(false);
            return;
        }

        let claude = self.claude.borrow().as_ref().and_then(|s| {
            let any = |f: fn(SessionKind) -> bool| s.sessions.iter().any(|v| f(v.kind));
            if any(|k| matches!(k, SessionKind::Permission | SessionKind::NeedsYou)) {
                Some(ClaudeState::Waiting)
            } else if any(|k| k == SessionKind::Working) {
                Some(ClaudeState::Working)
            } else if any(|k| k == SessionKind::Done) {
                Some(ClaudeState::Done)
            } else {
                None
            }
        });
        let mood = mascot::mood(claude, self.music_playing(), self.away.get(), now.hour());

        // Au repos, au centre ; à la place du point quand Claude a la pilule ;
        // à droite quand c'est la musique (pochette à gauche) ; cachée sinon.
        let winner = self.arbiter.borrow().winner();
        let media_owner = self.media.borrow().as_ref().map(|(owner, _)| *owner);
        let place = match &winner {
            None => "center",
            Some(w) if w.module == bw_claude::MODULE_ID => "left",
            Some(w) if Some(w.module.as_str()) == media_owner => "right",
            Some(_) => "",
        };
        let animated = config.always_animated
            || (mood.is_lively() && (mood != mascot::Mood::Music || config.music));
        self.ui.set_mascot_place(place.into());
        self.ui.set_mascot_mood(mood.name().into());
        self.ui.set_mascot_animated(animated);
    }

    /// Endort la mascotte après `sleep_after_minutes` sans saisie. Le minuteur
    /// vise l'instant où ce délai sera atteint, et se recale s'il y a eu une
    /// saisie entre-temps.
    fn schedule_away(&self) {
        let config = self.mascot_config();
        if !config.enabled {
            self.away_timer.stop();
            return;
        }
        let limit = Duration::from_secs(u64::from(config.sleep_after_minutes) * 60);
        let idle = platform::idle_for();
        if idle >= limit {
            self.set_away(true);
            return;
        }
        self.away_timer.start(
            TimerMode::SingleShot,
            limit - idle + Duration::from_secs(1),
            || post(|c| c.schedule_away()),
        );
    }

    fn set_away(&self, away: bool) {
        if self.away.replace(away) == away {
            return;
        }
        log::info!("mascotte : {}", if away { "endormie" } else { "réveillée" });
        if away {
            self.with_platform(Platform::watch_user_return);
        }
        self.update_mascot();
    }

    // --- Gestes ------------------------------------------------------------

    fn gestures_config(&self) -> GesturesConfig {
        GesturesConfig::from_table(self.config.borrow().modules.get("gestures")).unwrap_or_default()
    }

    /// Molette ou pavé tactile sur l'île (pixels logiques ; `dy` > 0 : vers le
    /// haut). Maj + molette vaut un défilement horizontal.
    fn on_wheel(self: &Rc<Self>, dx: f32, dy: f32, shift: bool) {
        let config = self.gestures_config();
        if !config.enabled {
            return;
        }
        let (dx, dy) = if shift && dx == 0.0 {
            (dy, 0.0)
        } else {
            (dx, dy)
        };
        let found = self
            .wheel
            .borrow_mut()
            .feed(dx, dy, std::time::Instant::now());
        for gesture in found {
            self.run_gesture(gesture, &config);
        }
    }

    fn on_drag(self: &Rc<Self>, dx: f32) {
        let config = self.gestures_config();
        if let Some(gesture) = gestures::drag(dx).filter(|_| config.enabled) {
            self.run_gesture(gesture, &config);
        }
    }

    fn run_gesture(self: &Rc<Self>, gesture: Gesture, config: &GesturesConfig) {
        match gesture {
            Gesture::VolumeUp | Gesture::VolumeDown => {
                let sign = if gesture == Gesture::VolumeUp {
                    1.0
                } else {
                    -1.0
                };
                match bw_volume::nudge(sign * config.volume_step as f32 / 100.0) {
                    Ok((level, muted)) => {
                        // Le module volume affiche déjà ses changements.
                        let shown = self.module_ids.borrow().contains(&bw_volume::MODULE_ID);
                        self.gesture_feedback(&bw_volume::label(level, muted), !shown);
                    }
                    Err(e) => log::warn!("geste : volume : {e:#}"),
                }
            }
            Gesture::Next | Gesture::Previous => {
                if self.media.borrow().is_some() {
                    let next = gesture == Gesture::Next;
                    self.media_action(if next { "next" } else { "prev" }.to_owned());
                    let text = if next {
                        tr!("Next track", "Morceau suivant")
                    } else {
                        tr!("Previous track", "Morceau précédent")
                    };
                    self.gesture_feedback(&text, true);
                }
            }
        }
    }

    fn on_long_press(self: &Rc<Self>) {
        if !self.gestures_config().enabled {
            return;
        }
        let on = !self.dnd.get();
        self.set_dnd(on);
        let text = if on {
            tr!("Do not disturb on", "Ne pas déranger activé")
        } else {
            tr!("Do not disturb off", "Ne pas déranger désactivé")
        };
        self.gesture_feedback(&text, true);
    }

    /// Retour d'un geste : dans l'en-tête si l'île est ouverte, sinon dans la
    /// pilule (`compact`).
    fn gesture_feedback(self: &Rc<Self>, text: &str, compact: bool) {
        if self.expanded.get() {
            self.ui.set_toast(text.into());
            let weak = Rc::downgrade(self);
            self.toast_timer.start(
                TimerMode::SingleShot,
                Duration::from_millis(1500),
                move || {
                    if let Some(c) = weak.upgrade() {
                        c.ui.set_toast("".into());
                    }
                },
            );
        } else if compact {
            self.flash(text);
        }
    }

    // --- Mode présentation --------------------------------------------------

    fn presentation_config(&self) -> bw_presence::PresentationConfig {
        bw_presence::PresentationConfig::from_table(
            self.config.borrow().modules.get(bw_presence::MODULE_ID),
        )
        .unwrap_or_default()
    }

    /// L'île hors des captures et partages d'écran, si demandé.
    fn apply_capture_exclusion(&self) {
        let hide = self.presentation_config().hide_from_capture;
        self.with_platform(|p| p.set_capture_excluded(hide));
    }

    fn apply_live(&self, snapshot: &bw_presence::LiveSnapshot) {
        self.live.set(snapshot.live());
        self.ui.set_live(snapshot.live());
        self.refresh_shape();
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
                let relevant = self.platform.borrow().as_ref().is_some_and(|p| {
                    p.foreground_on_our_monitor() && !p.foreground_is_capture_tool()
                });
                self.fullscreen.set(entering && relevant);
                self.update_visibility();
            }
            // Le calque de capture peut prendre le premier plan après
            // l'annonce du plein écran : l'île réapparaît alors.
            PlatformEvent::Foreground => {
                if self.fullscreen.get()
                    && self
                        .platform
                        .borrow()
                        .as_ref()
                        .is_some_and(Platform::foreground_is_capture_tool)
                {
                    self.fullscreen.set(false);
                    self.update_visibility();
                }
            }
            PlatformEvent::TaskbarChanged => self.update_taskbar(),
            PlatformEvent::UserReturned => {
                self.set_away(false);
                self.schedule_away();
            }
            PlatformEvent::DisplayChanged => {
                self.place();
                self.update_taskbar();
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
            TrayCommand::Settings => self.open_settings(),
            TrayCommand::ClaudeHooks => self.toggle_claude_hooks(),
            TrayCommand::Update => self.update_command(),
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
    if NotifyModule::is_supported() && config.module_enabled(bw_notify::MODULE_ID, true) {
        match NotifyConfig::from_table(config.modules.get(bw_notify::MODULE_ID)) {
            Ok(c) => modules.push(Box::new(NotifyModule::new(c))),
            Err(e) => errors.push(format!("modules.notifications : {e:#}")),
        }
    }
    if config.module_enabled(bw_calendar::MODULE_ID, true) {
        match CalendarConfig::from_table(config.modules.get(bw_calendar::MODULE_ID)) {
            Ok(c) => modules.push(Box::new(CalendarModule::new(c))),
            Err(e) => errors.push(format!("modules.calendar : {e:#}")),
        }
    }
    if config.module_enabled(bw_timer::MODULE_ID, false) {
        match TimerConfig::from_table(config.modules.get(bw_timer::MODULE_ID)) {
            Ok(c) => modules.push(Box::new(TimerModule::new(c))),
            Err(e) => errors.push(format!("modules.timer : {e:#}")),
        }
    }
    if bw_volume::VolumeModule::is_supported() && config.module_enabled(bw_volume::MODULE_ID, false)
    {
        match bw_volume::VolumeConfig::from_table(config.modules.get(bw_volume::MODULE_ID)) {
            Ok(c) => modules.push(Box::new(bw_volume::VolumeModule::new(c))),
            Err(e) => errors.push(format!("modules.volume : {e:#}")),
        }
    }
    if bw_volume::VolumeModule::is_supported()
        && config.module_enabled(bw_volume::BRIGHTNESS_ID, false)
    {
        let table = config.modules.get(bw_volume::BRIGHTNESS_ID);
        match bw_volume::VolumeConfig::from_module_table(bw_volume::BRIGHTNESS_ID, table) {
            Ok(c) => modules.push(Box::new(bw_volume::VolumeModule::brightness(c))),
            Err(e) => errors.push(format!("modules.brightness : {e:#}")),
        }
    }
    // Le visualiseur sert aussi à faire danser la mascotte sur la musique.
    let mascot = MascotConfig::from_table(config.modules.get("mascot")).unwrap_or_default();
    let dance = mascot.enabled && mascot.music;
    if bw_viz::VizModule::is_supported()
        && (config.module_enabled(bw_viz::MODULE_ID, false) || dance)
    {
        match bw_viz::VizConfig::from_table(config.modules.get(bw_viz::MODULE_ID)) {
            Ok(c) => modules.push(Box::new(bw_viz::VizModule::new(c))),
            Err(e) => errors.push(format!("modules.visualizer : {e:#}")),
        }
    }
    if config.module_enabled(bw_claude::ACTIVITY_ID, true) {
        match bw_claude::ActivityConfig::from_table(config.modules.get(bw_claude::ACTIVITY_ID)) {
            Ok(c) => modules.push(Box::new(bw_claude::ActivityModule::new(c))),
            Err(e) => errors.push(format!("modules.claude_activity : {e:#}")),
        }
    }
    if config.module_enabled(bw_plugins::MODULE_ID, false) {
        match bw_plugins::PluginsConfig::from_table(config.modules.get(bw_plugins::MODULE_ID)) {
            Ok(c) => modules.push(Box::new(bw_plugins::PluginsModule::new(
                c,
                bw_plugins::plugins_dir(&bw_config::config_dir()),
            ))),
            Err(e) => errors.push(format!("modules.plugins : {e:#}")),
        }
    }
    for id in [bw_power::BATTERY_ID, bw_power::BLUETOOTH_ID] {
        if bw_power::PowerModule::is_supported() && config.module_enabled(id, true) {
            match bw_power::PowerConfig::from_table(id, config.modules.get(id)) {
                Ok(c) if id == bw_power::BATTERY_ID => {
                    modules.push(Box::new(bw_power::PowerModule::battery(c)));
                }
                Ok(c) => modules.push(Box::new(bw_power::PowerModule::bluetooth(c))),
                Err(e) => errors.push(format!("modules.{id} : {e:#}")),
            }
        }
    }
    if bw_presence::PresenceModule::is_supported()
        && config.module_enabled(bw_presence::MODULE_ID, true)
    {
        match bw_presence::PresentationConfig::from_table(
            config.modules.get(bw_presence::MODULE_ID),
        ) {
            Ok(c) => modules.push(Box::new(bw_presence::PresenceModule::new(c))),
            Err(e) => errors.push(format!("modules.presentation : {e:#}")),
        }
    }
    if config.module_enabled(bw_weather::MODULE_ID, false) {
        match bw_weather::WeatherConfig::from_table(config.modules.get(bw_weather::MODULE_ID)) {
            Ok(c) => modules.push(Box::new(bw_weather::WeatherModule::new(c))),
            Err(e) => errors.push(format!("modules.weather : {e:#}")),
        }
    }
    if config.module_enabled("demo", false) {
        modules.push(Box::new(demo::Demo::default()));
    }
    (modules, errors)
}

/// Partage `budget` lignes entre l'agenda et Claude : chacun garde au moins
/// la moitié s'il en a besoin, l'autre récupère le reste.
fn split_rows(agenda: usize, claude: usize, budget: usize) -> (usize, usize) {
    let half = budget.div_ceil(2);
    let a = agenda.min(if claude == 0 {
        budget
    } else {
        half.max(budget - claude.min(budget))
    });
    let c = claude.min(budget - a);
    (a, c)
}

/// « 3:07 », « 1:02:45 ».
/// « 950 », « 123k », « 1.2M » : lisible dans une ligne étroite.
fn compact_tokens(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{}k", n / 1_000),
        _ => format!("{:.1}M", n as f64 / 1_000_000.0),
    }
}

/// « 2 h 10 » ou « 45 min ».
fn duration_text(d: Duration) -> String {
    let minutes = d.as_secs().div_ceil(60);
    if minutes >= 60 {
        format!("{} h {:02}", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} min")
    }
}

/// « à l'instant », « il y a 12 min », « il y a 3 h », « il y a 2 j ».
fn ago(then: chrono::DateTime<chrono::Utc>, now: chrono::DateTime<chrono::Utc>) -> String {
    let minutes = (now - then).num_minutes().max(0);
    match minutes {
        0 => tr!("just now", "à l'instant"),
        1..=59 => tr!("{minutes} min ago", "il y a {minutes} min"),
        60..=1439 => {
            let hours = minutes / 60;
            tr!("{hours} h ago", "il y a {hours} h")
        }
        _ => {
            let days = minutes / 1440;
            tr!("{days} d ago", "il y a {days} j")
        }
    }
}

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

fn rgb([r, g, b]: [u8; 3]) -> slint::Color {
    slint::Color::from_rgb_u8(r, g, b)
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

/// Langue de l'interface choisie dans la config.
fn language(setting: bw_config::Language) -> bw_i18n::Lang {
    match setting {
        bw_config::Language::Auto => bw_i18n::Lang::system(),
        bw_config::Language::En => bw_i18n::Lang::En,
        bw_config::Language::Fr => bw_i18n::Lang::Fr,
    }
}

/// Aligne les textes Slint (`@tr`) sur la langue courante. L'anglais est la
/// langue source des fichiers `.slint` : « "" » la sélectionne.
fn select_ui_language() {
    let code = match bw_i18n::lang() {
        bw_i18n::Lang::Fr => "fr",
        bw_i18n::Lang::En => "",
    };
    if let Err(e) = slint::select_bundled_translation(code) {
        log::warn!("langue de l'interface : {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::{ago, compact_tokens, duration_text, split_rows};

    #[test]
    fn activity_formats() {
        use chrono::{Duration as D, TimeZone, Utc};
        bw_i18n::set(bw_i18n::Lang::Fr);
        assert_eq!(compact_tokens(950), "950");
        assert_eq!(compact_tokens(123_456), "123k");
        assert_eq!(compact_tokens(1_250_000), "1.2M");
        assert_eq!(
            duration_text(std::time::Duration::from_secs(45 * 60)),
            "45 min"
        );
        assert_eq!(
            duration_text(std::time::Duration::from_secs(130 * 60)),
            "2 h 10"
        );
        let now = Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap();
        assert_eq!(ago(now - D::seconds(20), now), "à l'instant");
        assert_eq!(ago(now - D::minutes(12), now), "il y a 12 min");
        assert_eq!(ago(now - D::hours(3), now), "il y a 3 h");
        assert_eq!(ago(now - D::days(2), now), "il y a 2 j");
    }

    #[test]
    fn rows_are_shared() {
        assert_eq!(split_rows(5, 0, 4), (4, 0));
        assert_eq!(split_rows(0, 5, 4), (0, 4));
        assert_eq!(split_rows(5, 5, 4), (2, 2));
        assert_eq!(split_rows(1, 5, 4), (1, 3));
        assert_eq!(split_rows(5, 1, 4), (3, 1));
        assert_eq!(split_rows(3, 3, 2), (1, 1));
        assert_eq!(split_rows(0, 0, 2), (0, 0));
    }
}
