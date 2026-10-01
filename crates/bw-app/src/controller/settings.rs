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
use bw_media::MediaConfig;
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};

use super::{Controller, build_modules, post};
use crate::platform;
use crate::{CalendarSourceRow, SettingsWindow};

const DOCS: &str = "https://yazouv.github.io/BoringWindows/";

/// Services proposés par l'assistant : (nom, page de la doc, aide courte, fichier ?).
const PROVIDERS: [(&str, &str, &str, bool); 6] = [
    (
        "Google Agenda",
        "agenda/google.html",
        "Sur calendar.google.com : ⚙️ Paramètres › ton agenda (à gauche) › « Intégrer l'agenda » › copie l'« Adresse secrète au format iCal » (elle finit par basic.ics).",
        false,
    ),
    (
        "Outlook / Microsoft 365",
        "agenda/outlook.html",
        "Sur Outlook web : ⚙️ Paramètres › Calendrier › Calendriers partagés › « Publier un calendrier » › choisis « Peut afficher tous les détails » › Publier › copie le lien ICS.",
        false,
    ),
    (
        "iCloud",
        "agenda/icloud.html",
        "Dans Calendrier (iPhone, Mac ou iCloud.com) : partage du calendrier › active « Calendrier public » › copie le lien (webcal://…).",
        false,
    ),
    (
        "Proton Calendar",
        "agenda/proton.html",
        "Sur calendar.proton.me : ⚙️ Paramètres › Calendriers › ton calendrier › « Partager avec n'importe qui » › Créer un lien (tous les détails) › copie-le.",
        false,
    ),
    (
        "Emploi du temps, autre lien ICS",
        "agenda/autres.html",
        "Cherche « iCal », « ICS », « Exporter » ou « S'abonner » sur la page de ton emploi du temps ou de ton service : colle le lien (souvent en .ics ou webcal://).",
        false,
    ),
    (
        "Fichier .ics sur le disque",
        "agenda/autres.html#fichier-ics-sur-ton-disque",
        "Choisis un fichier .ics exporté ou reçu par mail. Il est relu régulièrement : remplace-le pour le mettre à jour.",
        true,
    ),
];

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
    }

    fn fill_settings(&self, ui: &SettingsWindow) {
        let config = self.config.borrow();
        let g = &config.general;
        ui.set_open_on_index(i32::from(g.open_on == bw_config::OpenOn::Click));
        ui.set_monitor_index(i32::from(g.monitor == bw_config::MonitorChoice::Cursor));
        ui.set_hide_fullscreen(g.hide_in_fullscreen);
        ui.set_autostart(platform::autostart_enabled());

        let t = &config.theme;
        ui.set_accent(hex(t.accent).into());
        ui.set_accent_preview(super::color(t.accent));
        ui.set_background_color(hex(t.background).into());
        ui.set_animation_ms(t.animation_ms as i32);
        ui.set_corner_radius(t.corner_radius.round() as i32);

        let cal = CalendarConfig::from_table(config.modules.get(bw_calendar::MODULE_ID))
            .unwrap_or_default();
        ui.set_remind_minutes(cal.remind_minutes as i32);
        ui.set_lookahead_hours(cal.lookahead_hours as i32);
        ui.set_refresh_minutes(cal.refresh_minutes as i32);
        ui.set_show_all_day(cal.show_all_day);
        ui.set_providers(ModelRc::new(VecModel::from(
            PROVIDERS
                .iter()
                .map(|p| SharedString::from(p.0))
                .collect::<Vec<_>>(),
        )));
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
        drop(config);

        self.refresh_sources(ui);
    }

    fn refresh_sources(&self, ui: &SettingsWindow) {
        let rows: Vec<CalendarSourceRow> = ConfigEditor::open(&self.path)
            .map(|e| e.calendar_sources())
            .unwrap_or_default()
            .into_iter()
            .map(|(name, url)| CalendarSourceRow {
                name: name.into(),
                url: url.into(),
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
            let page = PROVIDERS
                .get(ui.get_provider_index() as usize)
                .map_or("agenda/index.html", |p| p.1);
            platform::open_path(std::path::Path::new(&format!("{DOCS}{page}")));
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
            ui.set_testing(true);
            ui.set_test_status("".into());
            std::thread::spawn(move || {
                let result = bw_calendar::probe(&url);
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
            match c.edit_now(|e| e.add_calendar_source(&name, &url)) {
                Ok(()) => {
                    ui.set_new_name("".into());
                    ui.set_new_url("".into());
                    ui.set_test_status("".into());
                    c.refresh_sources(ui);
                    status(
                        ui,
                        "Calendrier ajouté ✓ (il apparaît dans l'île d'ici quelques secondes)",
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
                match c.edit_now(|e| e.remove_calendar_source(i as usize)) {
                    Ok(()) => {
                        c.refresh_sources(&s.ui);
                        status(&s.ui, "Calendrier retiré ✓", false);
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
        ui.on_open_config_file(with(|c, _| platform::open_path(&c.path)));
        ui.on_open_docs(with(|_, _| platform::open_path(std::path::Path::new(DOCS))));

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

    /// Une valeur a changé dans la fenêtre : on la met en attente d'écriture.
    fn settings_changed(self: &Rc<Self>, key: &str) {
        let settings = self.settings.borrow();
        let Some(s) = settings.as_ref() else { return };
        let ui = &s.ui;

        let change: Option<(Vec<&'static str>, Value)> = match key {
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
            "general.hide_in_fullscreen" => Some((
                vec!["general", "hide_in_fullscreen"],
                Value::Bool(ui.get_hide_fullscreen()),
            )),
            "autostart" => {
                let enabled = ui.get_autostart();
                match platform::set_autostart(enabled) {
                    Ok(()) => {
                        if let Some(tray) = self.tray.borrow().as_ref() {
                            tray.set_autostart_checked(enabled);
                        }
                        status(ui, "Enregistré ✓", false);
                    }
                    Err(e) => {
                        ui.set_autostart(!enabled);
                        status(ui, &format!("Démarrage automatique : {e}"), true);
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
                        status(ui, "Couleur au format #RRGGBB (ex. #FF8A3D)", true);
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

    /// Écrit les modifications en attente.
    fn flush_settings(&self) {
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
                Ok(()) => status(&s.ui, "Enregistré ✓", false),
                Err(e) => status(&s.ui, &e, true),
            }
        }
    }

    /// Relit config.toml, applique `f`, vérifie le tout (modules compris)
    /// et enregistre. Le rechargement à chaud applique ensuite les réglages.
    fn edit_now(&self, f: impl FnOnce(&mut ConfigEditor)) -> Result<(), String> {
        let mut editor = ConfigEditor::open(&self.path).map_err(|e| e.to_string())?;
        f(&mut editor);
        let config = Config::from_toml_str(&editor.text()).map_err(|e| e.to_string())?;
        let (_, errors) = build_modules(&config);
        if let Some(e) = errors.into_iter().next() {
            return Err(e);
        }
        editor.save().map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn set_provider(ui: &SettingsWindow, index: i32) {
    let p = PROVIDERS.get(index as usize).unwrap_or(&PROVIDERS[0]);
    ui.set_provider_index(index);
    ui.set_provider_help(p.2.into());
    ui.set_provider_is_file(p.3);
}

/// Nom proposé quand l'utilisateur n'en donne pas : le service, ou le nom du
/// fichier pour un lien générique (« G7a » pour …/G7a.ics).
fn default_name(provider: i32, url: &str) -> String {
    match provider {
        0 => "Google".into(),
        1 => "Outlook".into(),
        2 => "iCloud".into(),
        3 => "Proton".into(),
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
                "Agenda".into()
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
            "✓ Calendrier lu, mais aucun événement dans les 30 prochains jours".into(),
        ),
        Ok(p) => (
            true,
            format!(
                "✓ {} événement(s) dans les 30 jours{}",
                p.upcoming,
                p.next
                    .map(|n| format!(" · prochain : {n}"))
                    .unwrap_or_default()
            ),
        ),
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
