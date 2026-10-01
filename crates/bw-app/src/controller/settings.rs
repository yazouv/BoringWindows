//! Fenêtre de réglages : lit la config courante, enregistre chaque
//! modification dans config.toml (commentaires conservés), et pilote
//! l'assistant « Ajouter un calendrier ».

use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use std::time::Duration;

use bw_calendar::CalendarConfig;
use bw_claude::ClaudeConfig;
use bw_config::{Config, ConfigEditor, Value};
use bw_i18n::{Lang, tr};
use bw_media::MediaConfig;
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};

use super::{Controller, build_modules, post};
use crate::platform;
use crate::{CalendarSourceRow, PlayerRow, SettingsWindow};

fn ignored_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Services proposés par l'assistant, dans l'ordre de la liste.
const PROVIDERS: usize = 7;
/// CalDAV : serveur + identifiant + mot de passe d'application.
const CALDAV_PROVIDER: i32 = 6;
/// Le dernier service est un fichier local.
const FILE_PROVIDER: i32 = 5;

/// Adresse de la doc dans la langue courante.
fn docs() -> &'static str {
    match bw_i18n::lang() {
        Lang::Fr => "https://yazouv.github.io/BoringWindows/",
        Lang::En => "https://yazouv.github.io/BoringWindows/en/",
    }
}

fn provider_name(index: i32) -> String {
    match index {
        0 => tr!("Google Calendar", "Google Agenda"),
        1 => "Outlook / Microsoft 365".into(),
        2 => "iCloud".into(),
        3 => "Proton Calendar".into(),
        4 => tr!(
            "School timetable, other ICS link",
            "Emploi du temps, autre lien ICS"
        ),
        CALDAV_PROVIDER => "CalDAV (iCloud, Fastmail, Nextcloud…)".into(),
        _ => tr!(".ics file on disk", "Fichier .ics sur le disque"),
    }
}

/// Page de la doc qui détaille le service.
fn provider_page(index: i32) -> String {
    match index {
        0 => "agenda/google.html".into(),
        1 => "agenda/outlook.html".into(),
        2 => "agenda/icloud.html".into(),
        3 => "agenda/proton.html".into(),
        4 => "agenda/autres.html".into(),
        CALDAV_PROVIDER => "agenda/caldav.html".into(),
        _ => tr!(
            "agenda/autres.html#ics-file-on-your-disk",
            "agenda/autres.html#fichier-ics-sur-ton-disque"
        ),
    }
}

/// Aide courte : où trouver le lien.
fn provider_help(index: i32) -> String {
    match index {
        0 => tr!(
            "On calendar.google.com: ⚙️ Settings › your calendar (left) › \"Integrate calendar\" › copy the \"Secret address in iCal format\" (it ends with basic.ics).",
            "Sur calendar.google.com : ⚙️ Paramètres › ton agenda (à gauche) › « Intégrer l'agenda » › copie l'« Adresse secrète au format iCal » (elle finit par basic.ics)."
        ),
        1 => tr!(
            "On Outlook on the web: ⚙️ Settings › Calendar › Shared calendars › \"Publish a calendar\" › choose \"Can view all details\" › Publish › copy the ICS link.",
            "Sur Outlook web : ⚙️ Paramètres › Calendrier › Calendriers partagés › « Publier un calendrier » › choisis « Peut afficher tous les détails » › Publier › copie le lien ICS."
        ),
        2 => tr!(
            "In Calendar (iPhone, Mac or iCloud.com): share the calendar › turn on \"Public Calendar\" › copy the link (webcal://…).",
            "Dans Calendrier (iPhone, Mac ou iCloud.com) : partage du calendrier › active « Calendrier public » › copie le lien (webcal://…)."
        ),
        3 => tr!(
            "On calendar.proton.me: ⚙️ Settings › Calendars › your calendar › \"Share with anyone\" › Create link (full details) › copy it.",
            "Sur calendar.proton.me : ⚙️ Paramètres › Calendriers › ton calendrier › « Partager avec n'importe qui » › Créer un lien (tous les détails) › copie-le."
        ),
        4 => tr!(
            "Look for \"iCal\", \"ICS\", \"Export\" or \"Subscribe\" on your timetable or service page, and paste the link (often ending in .ics, or webcal://).",
            "Cherche « iCal », « ICS », « Exporter » ou « S'abonner » sur la page de ton emploi du temps ou de ton service : colle le lien (souvent en .ics ou webcal://)."
        ),
        CALDAV_PROVIDER => tr!(
            "Server address of your account (iCloud: https://caldav.icloud.com, Fastmail: https://caldav.fastmail.com, Nextcloud: https://your-server/remote.php/dav) + your login + an app password. All your calendars are read; the password goes to the Windows Credential Manager.",
            "Adresse du serveur de ton compte (iCloud : https://caldav.icloud.com, Fastmail : https://caldav.fastmail.com, Nextcloud : https://ton-serveur/remote.php/dav) + ton identifiant + un mot de passe d'application. Tous tes agendas sont lus ; le mot de passe va dans le Gestionnaire d'identifiants Windows."
        ),
        _ => tr!(
            "Pick an .ics file you exported or received by email. It is re-read regularly: replace it to update it.",
            "Choisis un fichier .ics exporté ou reçu par mail. Il est relu régulièrement : remplace-le pour le mettre à jour."
        ),
    }
}

/// État de la fenêtre ouverte.
pub(super) struct SettingsState {
    ui: SettingsWindow,
    /// Modifications en attente d'écriture (regroupées pendant la frappe).
    pending: RefCell<Vec<(Vec<&'static str>, Value)>>,
    save_timer: Timer,
}

impl Controller {
    pub(super) fn open_settings(self: &Rc<Self>) {
        if let Some(s) = self.settings.borrow().as_ref() {
            let _ = s.ui.show();
            return;
        }
        let ui = match SettingsWindow::new() {
            Ok(ui) => ui,
            Err(e) => {
                log::error!("fenêtre de réglages : {e}");
                return;
            }
        };
        self.fill_settings(&ui);
        self.wire_settings(&ui);
        let _ = ui.show();
        *self.settings.borrow_mut() = Some(SettingsState {
            ui,
            pending: RefCell::new(Vec::new()),
            save_timer: Timer::default(),
        });
        self.sync_settings_update(self.update_state.get());
    }

    fn fill_settings(&self, ui: &SettingsWindow) {
        let config = self.config.borrow();
        let g = &config.general;
        ui.set_open_on_index(i32::from(g.open_on == bw_config::OpenOn::Click));
        ui.set_monitor_index(i32::from(g.monitor == bw_config::MonitorChoice::Cursor));
        ui.set_hide_fullscreen(g.hide_in_fullscreen);
        ui.set_language_index(language_index(g.language));
        ui.set_auto_update(g.auto_update);
        ui.set_app_version(super::update::CURRENT.into());
        ui.set_autostart(platform::autostart_enabled());

        self.fill_themes(ui, &config.theme.name);
        fill_appearance(ui, &config.theme);
        fill_layout(ui, &config.layout.compact);
        ui.set_view_name(config.layout.view.as_str().into());
        ui.set_volume_enabled(config.module_enabled(bw_volume::MODULE_ID, false));
        ui.set_viz_enabled(config.module_enabled(bw_viz::MODULE_ID, false));
        ui.set_plugins_enabled(config.module_enabled(bw_plugins::MODULE_ID, false));
        let shelf =
            crate::shelf::ShelfConfig::from_table(config.modules.get("shelf")).unwrap_or_default();
        ui.set_shelf_enabled(shelf.enabled);
        ui.set_shelf_max(shelf.max as i32);

        let cal = CalendarConfig::from_table(config.modules.get(bw_calendar::MODULE_ID))
            .unwrap_or_default();
        ui.set_remind_minutes(cal.remind_minutes as i32);
        ui.set_lookahead_hours(cal.lookahead_hours as i32);
        ui.set_refresh_minutes(cal.refresh_minutes as i32);
        ui.set_show_all_day(cal.show_all_day);
        fill_providers(ui);
        set_provider(ui, 0);

        let claude =
            ClaudeConfig::from_table(config.modules.get(bw_claude::MODULE_ID)).unwrap_or_default();
        ui.set_hooks_installed(self.installer.is_installed());
        ui.set_claude_permissions(claude.permissions);
        ui.set_claude_sound(claude.sound);
        ui.set_claude_wait(claude.permission_wait_secs as i32);

        let media =
            MediaConfig::from_table(config.modules.get(bw_media::MODULE_ID)).unwrap_or_default();
        ui.set_media_accent(media.accent_from_artwork);
        ui.set_media_ignore(media.ignore.join(", ").into());

        let timer = bw_timer::TimerConfig::from_table(config.modules.get(bw_timer::MODULE_ID))
            .unwrap_or_default();
        ui.set_timer_enabled(config.module_enabled(bw_timer::MODULE_ID, false));
        ui.set_timer_sound(timer.sound);
        ui.set_timer_presets(
            timer
                .presets
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
                .into(),
        );
        drop(config);

        self.refresh_sources(ui);
        self.refresh_players(ui);
    }

    /// Lecteurs vus depuis le lancement, plus ceux déjà ignorés (qui ne
    /// remontent plus), avec leur état coché.
    fn refresh_players(&self, ui: &SettingsWindow) {
        let ignored = ignored_list(&ui.get_media_ignore());
        let mut tokens: Vec<String> = self.media_seen.borrow().iter().cloned().collect();
        for i in &ignored {
            if !tokens.iter().any(|t| t.eq_ignore_ascii_case(i)) {
                tokens.push(i.to_ascii_lowercase());
            }
        }
        let rows: Vec<PlayerRow> = tokens
            .into_iter()
            .map(|t| PlayerRow {
                name: bw_media::display_name(&t).into(),
                ignored: ignored.iter().any(|i| i.eq_ignore_ascii_case(&t)),
                token: t.into(),
            })
            .collect();
        ui.set_players(ModelRc::new(VecModel::from(rows)));
    }

    fn move_layout(self: &Rc<Self>, index: usize, delta: i32) {
        self.flush_settings();
        let mut order = self.config.borrow().layout.compact.clone();
        let Some(target) = index.checked_add_signed(delta as isize) else {
            return;
        };
        if index >= order.len() || target >= order.len() {
            return;
        }
        order.swap(index, target);
        let result = self.edit_now(|e| e.set(&["layout", "compact"], Value::StrList(order)));
        let settings = self.settings.borrow();
        let Some(s) = settings.as_ref() else { return };
        match result {
            Ok(config) => {
                fill_layout(&s.ui, &config.layout.compact);
                status(&s.ui, &saved(), false);
            }
            Err(e) => status(&s.ui, &e, true),
        }
    }

    fn toggle_player(self: &Rc<Self>, token: &str, ignored: bool) {
        {
            let settings = self.settings.borrow();
            let Some(s) = settings.as_ref() else { return };
            let mut list = ignored_list(&s.ui.get_media_ignore());
            list.retain(|i| !i.eq_ignore_ascii_case(token));
            if ignored {
                list.push(token.to_owned());
            }
            s.ui.set_media_ignore(list.join(", ").into());
        }
        self.settings_changed("modules.media.ignore");
    }

    fn refresh_sources(&self, ui: &SettingsWindow) {
        let rows: Vec<CalendarSourceRow> = ConfigEditor::open(&self.path)
            .map(|e| e.calendar_sources())
            .unwrap_or_default()
            .into_iter()
            .map(|(name, url)| CalendarSourceRow {
                name: name.into(),
                url: if bw_secrets::is_reference(&url) {
                    tr!("🔒 Credential Manager", "🔒 Gestionnaire d'identifiants").into()
                } else {
                    url.into()
                },
            })
            .collect();
        ui.set_sources(ModelRc::new(VecModel::from(rows)));
    }

    fn wire_settings(self: &Rc<Self>, ui: &SettingsWindow) {
        let weak = Rc::downgrade(self);
        let with = move |f: fn(&Rc<Controller>, &SettingsWindow)| {
            let weak = weak.clone();
            move || {
                if let Some(c) = weak.upgrade()
                    && let Some(s) = c.settings.borrow().as_ref()
                {
                    f(&c, &s.ui);
                }
            }
        };

        let weak = Rc::downgrade(self);
        ui.on_module_moved(move |index, delta| {
            if let Some(c) = weak.upgrade() {
                c.move_layout(index as usize, delta);
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_player_toggled(move |token, ignored| {
            if let Some(c) = weak.upgrade() {
                c.toggle_player(&token, ignored);
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_changed(move |key| {
            if let Some(c) = weak.upgrade() {
                c.settings_changed(&key);
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_provider_changed(move |i| {
            if let Some(c) = weak.upgrade()
                && let Some(s) = c.settings.borrow().as_ref()
            {
                set_provider(&s.ui, i);
            }
        });
        ui.on_open_guide(with(|_, ui| {
            let page = provider_page(ui.get_provider_index());
            platform::open_path(std::path::Path::new(&format!("{}{page}", docs())));
        }));
        ui.on_browse_file(with(|_, ui| {
            if let Some(path) = platform::pick_ics_file() {
                ui.set_new_url(path.to_string_lossy().replace('\\', "/").into());
                if ui.get_new_name().is_empty()
                    && let Some(stem) = path.file_stem()
                {
                    ui.set_new_name(stem.to_string_lossy().as_ref().into());
                }
            }
        }));
        ui.on_test_source(with(|_, ui| {
            let url = ui.get_new_url().trim().to_owned();
            let source = bw_calendar::Source {
                name: String::new(),
                kind: if ui.get_provider_is_caldav() {
                    bw_calendar::SourceKind::Caldav
                } else {
                    bw_calendar::SourceKind::Ics
                },
                url,
                username: ui.get_new_username().trim().to_owned(),
                password: ui.get_new_password().to_string(),
            };
            ui.set_testing(true);
            ui.set_test_status("".into());
            std::thread::spawn(move || {
                let result = bw_calendar::probe_source(&source);
                post(move |c| {
                    if let Some(s) = c.settings.borrow().as_ref() {
                        s.ui.set_testing(false);
                        let (ok, text) = probe_text(result);
                        s.ui.set_test_ok(ok);
                        s.ui.set_test_status(text.into());
                    }
                });
            });
        }));
        ui.on_add_source(with(|c, ui| {
            let url = ui.get_new_url();
            let name = match ui.get_new_name().trim() {
                "" => default_name(ui.get_provider_index(), &url),
                n => n.to_owned(),
            };
            let caldav = ui.get_provider_is_caldav();
            let username = ui.get_new_username().trim().to_owned();
            // Liens privés et mots de passe : dans le coffre, pas dans config.toml.
            let stored_url = if caldav || !is_remote(&url) {
                url.trim().to_owned()
            } else {
                protect("ics", url.trim())
            };
            let stored_password = if caldav {
                protect("caldav", &ui.get_new_password())
            } else {
                String::new()
            };
            let result = c.edit_now(|e| {
                let account = caldav.then_some((username.as_str(), stored_password.as_str()));
                e.add_calendar_account(&name, &stored_url, account);
            });
            if result.is_err() {
                for value in [&stored_url, &stored_password] {
                    if let Some(id) = bw_secrets::reference_id(value) {
                        let _ = bw_secrets::delete(id);
                    }
                }
            }
            match result {
                Ok(_) => {
                    ui.set_new_name("".into());
                    ui.set_new_url("".into());
                    ui.set_new_username("".into());
                    ui.set_new_password("".into());
                    ui.set_test_status("".into());
                    c.refresh_sources(ui);
                    status(
                        ui,
                        &tr!(
                            "Calendar added ✓ (it shows up in the island within a few seconds)",
                            "Calendrier ajouté ✓ (il apparaît dans l'île d'ici quelques secondes)"
                        ),
                        false,
                    );
                }
                Err(e) => status(ui, &e, true),
            }
        }));
        let weak = Rc::downgrade(self);
        ui.on_remove_source(move |i| {
            if let Some(c) = weak.upgrade()
                && let Some(s) = c.settings.borrow().as_ref()
            {
                let secrets = ConfigEditor::open(&c.path)
                    .map(|e| e.calendar_source_secrets(i as usize))
                    .unwrap_or_default();
                match c.edit_now(|e| e.remove_calendar_source(i as usize)) {
                    Ok(_) => {
                        for value in secrets {
                            if let Some(id) = bw_secrets::reference_id(&value) {
                                let _ = bw_secrets::delete(id);
                            }
                        }
                        c.refresh_sources(&s.ui);
                        status(
                            &s.ui,
                            &tr!("Calendar removed ✓", "Calendrier retiré ✓"),
                            false,
                        );
                    }
                    Err(e) => status(&s.ui, &e, true),
                }
            }
        });
        ui.on_toggle_hooks(with(|c, ui| {
            c.toggle_claude_hooks();
            ui.set_hooks_installed(c.installer.is_installed());
        }));
        ui.on_run_doctor(with(|_, ui| {
            let Ok(exe) = std::env::current_exe() else {
                return;
            };
            ui.set_doctor_running(true);
            std::thread::spawn(move || {
                let report = bw_claude::doctor::run(&exe);
                post(move |c| {
                    if let Some(s) = c.settings.borrow().as_ref() {
                        s.ui.set_doctor_running(false);
                        s.ui.set_doctor_report(report.trim().into());
                    }
                });
            });
        }));
        ui.on_update_clicked(move || post(|c| c.update_command()));
        ui.on_theme_picked(move |_| {
            post(|c| c.pick_theme());
        });
        ui.on_open_themes_folder(with(|c, _| {
            let dir = bw_config::themes_dir(c.path.parent().unwrap_or(std::path::Path::new(".")));
            if let Err(e) = std::fs::create_dir_all(&dir) {
                log::warn!("dossier des thèmes : {e}");
            }
            platform::open_path(&dir);
        }));
        ui.on_open_plugins_folder(with(|_, _| {
            let dir = bw_plugins::plugins_dir(&bw_config::config_dir());
            if let Err(e) = std::fs::create_dir_all(&dir) {
                log::warn!("dossier des plugins : {e}");
            }
            platform::open_path(&dir);
        }));
        ui.on_shelf_clear(with(|c, _| {
            c.shelf.borrow_mut().clear();
            c.shelf.borrow().save(&c.shelf_file);
            c.update_shelf_ui();
        }));
        ui.on_open_layouts_folder(with(|c, _| {
            let dir = bw_config::layouts_dir(c.path.parent().unwrap_or(std::path::Path::new(".")));
            if let Err(e) = std::fs::create_dir_all(&dir) {
                log::warn!("dossier des vues : {e}");
            }
            platform::open_path(&dir);
        }));
        ui.on_open_config_file(with(|c, _| platform::open_path(&c.path)));
        ui.on_open_docs(with(|_, _| {
            platform::open_path(std::path::Path::new(docs()))
        }));

        let weak = Rc::downgrade(self);
        ui.window().on_close_requested(move || {
            if let Some(c) = weak.upgrade() {
                post(|c| {
                    c.flush_settings();
                    c.settings.borrow_mut().take();
                });
                drop(c);
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    /// Liste des thèmes (fournis puis personnels), `current` sélectionné.
    fn fill_themes(&self, ui: &SettingsWindow, current: &str) {
        let dir = self.path.parent().unwrap_or(std::path::Path::new("."));
        let ids = bw_config::available_themes(dir);
        let index = ids.iter().position(|id| id == current).unwrap_or(0);
        ui.set_themes(ModelRc::new(VecModel::from(
            ids.iter()
                .map(|id| SharedString::from(theme_label(id)))
                .collect::<Vec<_>>(),
        )));
        ui.set_theme_index(index as i32);
    }

    /// Langue changée : textes fournis par Rust (les `@tr` suivent seuls).
    pub(super) fn retranslate_settings(&self) {
        if let Some(s) = self.settings.borrow().as_ref() {
            let index = s.ui.get_provider_index();
            fill_providers(&s.ui);
            set_provider(&s.ui, index);
            s.ui.set_test_status("".into());
            s.ui.set_doctor_report("".into());
            s.ui.set_language_index(language_index(self.config.borrow().general.language));
            self.fill_themes(&s.ui, &self.config.borrow().theme.name);
        }
    }

    /// Une valeur a changé dans la fenêtre : on la met en attente d'écriture.
    fn settings_changed(self: &Rc<Self>, key: &str) {
        let settings = self.settings.borrow();
        let Some(s) = settings.as_ref() else { return };
        let ui = &s.ui;

        let change: Option<(Vec<&'static str>, Value)> = match key {
            "general.language" => Some((
                vec!["general", "language"],
                Value::Str(
                    match ui.get_language_index() {
                        1 => "en",
                        2 => "fr",
                        _ => "auto",
                    }
                    .into(),
                ),
            )),
            "general.open_on" => Some((
                vec!["general", "open_on"],
                Value::Str(
                    if ui.get_open_on_index() == 1 {
                        "click"
                    } else {
                        "hover"
                    }
                    .into(),
                ),
            )),
            "general.monitor" => Some((
                vec!["general", "monitor"],
                Value::Str(
                    if ui.get_monitor_index() == 1 {
                        "cursor"
                    } else {
                        "primary"
                    }
                    .into(),
                ),
            )),
            "general.auto_update" => Some((
                vec!["general", "auto_update"],
                Value::Bool(ui.get_auto_update()),
            )),
            "general.hide_in_fullscreen" => Some((
                vec!["general", "hide_in_fullscreen"],
                Value::Bool(ui.get_hide_fullscreen()),
            )),
            "autostart" => {
                let enabled = ui.get_autostart();
                match platform::set_autostart(enabled) {
                    Ok(_) => {
                        if let Some(tray) = self.tray.borrow().as_ref() {
                            tray.set_autostart_checked(enabled);
                        }
                        status(ui, &saved(), false);
                    }
                    Err(e) => {
                        ui.set_autostart(!enabled);
                        status(
                            ui,
                            &tr!("Start with Windows: {e}", "Démarrage automatique : {e}"),
                            true,
                        );
                    }
                }
                None
            }
            "theme.accent" | "theme.background" => {
                let text = if key == "theme.accent" {
                    ui.get_accent()
                } else {
                    ui.get_background_color()
                };
                match bw_config::Color::from_str(text.trim()) {
                    Ok(color) => {
                        if key == "theme.accent" {
                            ui.set_accent_preview(super::color(color));
                        }
                        let field = if key == "theme.accent" {
                            "accent"
                        } else {
                            "background"
                        };
                        Some((vec!["theme", field], Value::Str(text.trim().to_owned())))
                    }
                    Err(_) => {
                        status(
                            ui,
                            &tr!(
                                "Color as #RRGGBB (e.g. #FF8A3D)",
                                "Couleur au format #RRGGBB (ex. #FF8A3D)"
                            ),
                            true,
                        );
                        None
                    }
                }
            }
            "theme.animation_ms" => Some((
                vec!["theme", "animation_ms"],
                Value::Int(ui.get_animation_ms().into()),
            )),
            "theme.corner_radius" => Some((
                vec!["theme", "corner_radius"],
                Value::Float(ui.get_corner_radius().into()),
            )),
            "theme.compact" | "theme.expanded" => {
                let (table, w, h) = if key == "theme.compact" {
                    ("compact", ui.get_compact_w(), ui.get_compact_h())
                } else {
                    ("expanded", ui.get_expanded_w(), ui.get_expanded_h())
                };
                let mut pending = s.pending.borrow_mut();
                let width_path = vec!["theme", table, "width"];
                pending.retain(|(k, _)| *k != width_path);
                pending.push((width_path, Value::Int(w.into())));
                Some((vec!["theme", table, "height"], Value::Int(h.into())))
            }
            "modules.calendar.remind_minutes" => Some((
                vec!["modules", "calendar", "remind_minutes"],
                Value::Int(ui.get_remind_minutes().into()),
            )),
            "modules.calendar.lookahead_hours" => Some((
                vec!["modules", "calendar", "lookahead_hours"],
                Value::Int(ui.get_lookahead_hours().into()),
            )),
            "modules.calendar.refresh_minutes" => Some((
                vec!["modules", "calendar", "refresh_minutes"],
                Value::Int(ui.get_refresh_minutes().into()),
            )),
            "modules.calendar.show_all_day" => Some((
                vec!["modules", "calendar", "show_all_day"],
                Value::Bool(ui.get_show_all_day()),
            )),
            "modules.claude.permissions" => Some((
                vec!["modules", "claude", "permissions"],
                Value::Bool(ui.get_claude_permissions()),
            )),
            "modules.claude.sound" => Some((
                vec!["modules", "claude", "sound"],
                Value::Bool(ui.get_claude_sound()),
            )),
            "modules.claude.permission_wait_secs" => Some((
                vec!["modules", "claude", "permission_wait_secs"],
                Value::Int(ui.get_claude_wait().into()),
            )),
            "modules.media.accent_from_artwork" => Some((
                vec!["modules", "media", "accent_from_artwork"],
                Value::Bool(ui.get_media_accent()),
            )),
            "modules.plugins.enabled" => Some((
                vec!["modules", "plugins", "enabled"],
                Value::Bool(ui.get_plugins_enabled()),
            )),
            "modules.visualizer.enabled" => Some((
                vec!["modules", "visualizer", "enabled"],
                Value::Bool(ui.get_viz_enabled()),
            )),
            "modules.volume.enabled" => Some((
                vec!["modules", "volume", "enabled"],
                Value::Bool(ui.get_volume_enabled()),
            )),
            "modules.shelf.enabled" => Some((
                vec!["modules", "shelf", "enabled"],
                Value::Bool(ui.get_shelf_enabled()),
            )),
            "modules.shelf.max" => Some((
                vec!["modules", "shelf", "max"],
                Value::Int(ui.get_shelf_max().into()),
            )),
            "layout.view" => Some((
                vec!["layout", "view"],
                Value::Str(ui.get_view_name().trim().to_owned()),
            )),
            "modules.timer.enabled" => Some((
                vec!["modules", "timer", "enabled"],
                Value::Bool(ui.get_timer_enabled()),
            )),
            "modules.timer.sound" => Some((
                vec!["modules", "timer", "sound"],
                Value::Bool(ui.get_timer_sound()),
            )),
            "modules.timer.presets" => {
                let minutes: Vec<i64> = ui
                    .get_timer_presets()
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                // Saisie incomplète (« 5, » ou vide) : on attend la suite.
                (!minutes.is_empty())
                    .then(|| (vec!["modules", "timer", "presets"], Value::IntList(minutes)))
            }
            "modules.media.ignore" => Some((
                vec!["modules", "media", "ignore"],
                Value::StrList(
                    ui.get_media_ignore()
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect(),
                ),
            )),
            other => {
                log::warn!("réglage inconnu : {other}");
                None
            }
        };

        if let Some(change) = change {
            let mut pending = s.pending.borrow_mut();
            pending.retain(|(k, _)| *k != change.0);
            pending.push(change);
            // Regrouper les frappes : on écrit 300 ms après la dernière.
            s.save_timer
                .start(TimerMode::SingleShot, Duration::from_millis(300), || {
                    post(|c| c.flush_settings())
                });
        }
    }

    /// Thème choisi : il remplace les couleurs écrites dans `[theme]`.
    fn pick_theme(&self) {
        // Les réglages en attente d'abord, pour ne pas les perdre ni les
        // réécrire par-dessus le thème.
        self.flush_settings();
        let settings = self.settings.borrow();
        let Some(s) = settings.as_ref() else { return };
        let dir = self.path.parent().unwrap_or(std::path::Path::new("."));
        let ids = bw_config::available_themes(dir);
        let Some(id) = ids.get(s.ui.get_theme_index() as usize) else {
            return;
        };
        let result = self.edit_now(|e| {
            e.set(&["theme", "name"], Value::Str(id.clone()));
            for key in bw_config::THEME_KEYS {
                e.remove(&["theme", key]);
            }
        });
        match result {
            Ok(config) => {
                fill_appearance(&s.ui, &config.theme);
                status(&s.ui, &saved(), false);
            }
            Err(e) => status(&s.ui, &e, true),
        }
    }

    /// Message en bas de la fenêtre de réglages, si elle est ouverte.
    pub(super) fn settings_status(&self, text: &str, error: bool) {
        if let Some(s) = self.settings.borrow().as_ref() {
            status(&s.ui, text, error);
        }
    }

    pub(super) fn sync_settings_update(&self, state: super::update::UpdateState) {
        if let Some(s) = self.settings.borrow().as_ref() {
            s.ui.set_update_checking(state == super::update::UpdateState::Checking);
            s.ui.set_update_ready(matches!(state, super::update::UpdateState::Ready(_)));
        }
    }

    /// Écrit les modifications en attente.
    pub(super) fn flush_settings(&self) {
        let changes = match self.settings.borrow().as_ref() {
            Some(s) => std::mem::take(&mut *s.pending.borrow_mut()),
            None => return,
        };
        if changes.is_empty() {
            return;
        }
        let result = self.edit_now(|e| {
            for (key, value) in changes {
                e.set(&key, value);
            }
        });
        if let Some(s) = self.settings.borrow().as_ref() {
            match result {
                Ok(_) => status(&s.ui, &saved(), false),
                Err(e) => status(&s.ui, &e, true),
            }
        }
    }

    /// Relit config.toml, applique `f`, vérifie le tout (modules compris)
    /// et enregistre. Le rechargement à chaud applique ensuite les réglages.
    fn edit_now(&self, f: impl FnOnce(&mut ConfigEditor)) -> Result<Config, String> {
        let mut editor = ConfigEditor::open(&self.path).map_err(|e| e.to_string())?;
        f(&mut editor);
        let config = editor.config().map_err(|e| e.to_string())?;
        let (_, errors) = build_modules(&config);
        if let Some(e) = errors.into_iter().next() {
            return Err(e);
        }
        editor.save().map_err(|e| e.to_string())?;
        Ok(config)
    }
}

/// Nom affiché d'un thème : traduit pour ceux fournis, nom du fichier sinon.
fn theme_label(id: &str) -> String {
    match id {
        "default" => tr!("Black (default)", "Noir (par défaut)"),
        "light" => tr!("Light", "Clair"),
        "midnight" => tr!("Midnight", "Minuit"),
        "glass" => tr!("Smoked glass", "Verre fumé"),
        other => other.to_owned(),
    }
}

fn fill_appearance(ui: &SettingsWindow, t: &bw_config::Theme) {
    ui.set_accent(hex(t.accent).into());
    ui.set_accent_preview(super::color(t.accent));
    ui.set_background_color(hex(t.background).into());
    ui.set_animation_ms(t.animation_ms as i32);
    ui.set_corner_radius(t.corner_radius.round() as i32);
    ui.set_compact_w(t.compact.width.round() as i32);
    ui.set_compact_h(t.compact.height.round() as i32);
    ui.set_expanded_w(t.expanded.width.round() as i32);
    ui.set_expanded_h(t.expanded.height.round() as i32);
}

fn module_label(id: &str) -> String {
    match id {
        "claude" => "Claude Code".into(),
        "media" => tr!("Music", "Musique"),
        "calendar" => tr!("Calendar", "Agenda"),
        other => other.to_owned(),
    }
}

fn fill_layout(ui: &SettingsWindow, order: &[String]) {
    ui.set_module_order(ModelRc::new(VecModel::from(
        order
            .iter()
            .map(|id| SharedString::from(module_label(id)))
            .collect::<Vec<_>>(),
    )));
}

fn language_index(language: bw_config::Language) -> i32 {
    match language {
        bw_config::Language::Auto => 0,
        bw_config::Language::En => 1,
        bw_config::Language::Fr => 2,
    }
}

fn fill_providers(ui: &SettingsWindow) {
    ui.set_providers(ModelRc::new(VecModel::from(
        (0..PROVIDERS as i32)
            .map(|i| SharedString::from(provider_name(i)))
            .collect::<Vec<_>>(),
    )));
}

fn set_provider(ui: &SettingsWindow, index: i32) {
    ui.set_provider_index(index);
    ui.set_provider_help(provider_help(index).into());
    ui.set_provider_is_file(index == FILE_PROVIDER);
    ui.set_provider_is_caldav(index == CALDAV_PROVIDER);
}

fn is_remote(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("https://") || u.starts_with("http://") || u.starts_with("webcal://")
}

/// Identifiant de secret unique : `<préfixe>-<horodatage>`.
fn new_secret_id(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{prefix}-{nanos:x}")
}

/// Met `value` dans le coffre et renvoie la référence `secret:<id>` à écrire
/// dans la config ; sans coffre (ou en cas d'échec), la valeur elle-même.
fn protect(prefix: &str, value: &str) -> String {
    if !bw_secrets::available() {
        return value.to_owned();
    }
    let id = new_secret_id(prefix);
    match bw_secrets::set(&id, value) {
        Ok(()) => bw_secrets::reference(&id),
        Err(e) => {
            log::warn!("coffre à secrets indisponible : {e:#}");
            value.to_owned()
        }
    }
}

fn saved() -> String {
    tr!("Saved ✓", "Enregistré ✓")
}

/// Nom proposé quand l'utilisateur n'en donne pas : le service, ou le nom du
/// fichier pour un lien générique (« G7a » pour …/G7a.ics).
fn default_name(provider: i32, url: &str) -> String {
    match provider {
        0 => "Google".into(),
        1 => "Outlook".into(),
        2 => "iCloud".into(),
        3 => "Proton".into(),
        CALDAV_PROVIDER => "CalDAV".into(),
        _ => {
            let path = url.split(['?', '#']).next().unwrap_or_default();
            let last = path
                .trim_end_matches(['/', '\\'])
                .rsplit(['/', '\\'])
                .next();
            let stem = last.unwrap_or_default();
            let stem = stem
                .strip_suffix(".ics")
                .or_else(|| stem.strip_suffix(".ICS"))
                .unwrap_or(stem);
            if stem.is_empty() || stem.contains(':') {
                tr!("Calendar", "Agenda")
            } else {
                stem.into()
            }
        }
    }
}

fn probe_text(result: anyhow::Result<bw_calendar::Probe>) -> (bool, String) {
    match result {
        Ok(p) if p.upcoming == 0 => (
            true,
            tr!(
                "✓ Calendar read, but no event in the next 30 days",
                "✓ Calendrier lu, mais aucun événement dans les 30 prochains jours"
            ),
        ),
        Ok(p) => {
            let next = p
                .next
                .map(|n| tr!(" · next: {n}", " · prochain : {n}"))
                .unwrap_or_default();
            (
                true,
                tr!(
                    "✓ {} event(s) in the next 30 days{next}",
                    "✓ {} événement(s) dans les 30 jours{next}",
                    p.upcoming
                ),
            )
        }
        Err(e) => (false, format!("✗ {e:#}")),
    }
}

fn status(ui: &SettingsWindow, text: &str, error: bool) {
    ui.set_status(text.into());
    ui.set_status_error(error);
}

fn hex(c: bw_config::Color) -> String {
    if c.a == 0xFF {
        format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", c.r, c.g, c.b, c.a)
    }
}

#[cfg(test)]
mod tests {
    use super::default_name;

    #[test]
    fn default_names() {
        bw_i18n::set(bw_i18n::Lang::Fr);
        assert_eq!(
            default_name(0, "https://calendar.google.com/x/basic.ics"),
            "Google"
        );
        assert_eq!(
            default_name(
                4,
                "https://edt.example.fr/api/v1/years/3/calendar/G7a.ics?k=1"
            ),
            "G7a"
        );
        assert_eq!(default_name(5, r"C:\Users\moi\Cours.ics"), "Cours");
        assert_eq!(default_name(4, "webcal://"), "Agenda");
    }
}
