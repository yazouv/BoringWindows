//! Assistant d'accueil : au premier lancement (config.toml absent), ou depuis
//! Réglages › Général. Il propose la langue, les modules, l'installation des
//! hooks Claude Code et l'ajout d'un agenda.
//!
//! Les choix de modules ne sont écrits qu'à la fin, en une seule fois : chaque
//! écriture de config.toml redémarre les modules.

use std::rc::Rc;

use bw_config::Value;
use slint::ComponentHandle;

use super::Controller;
use crate::WelcomeWindow;

/// Modules proposés : (clé dans `[modules.<id>]`, actif sans réglage).
const MODULES: [(&str, bool); 10] = [
    (bw_claude::MODULE_ID, true),
    (bw_media::MODULE_ID, true),
    (bw_calendar::MODULE_ID, true),
    (bw_notify::MODULE_ID, true),
    (bw_weather::MODULE_ID, false),
    (bw_timer::MODULE_ID, false),
    ("tasks", false),
    ("mascot", true),
    (bw_network::MODULE_ID, true),
    (bw_system::MODULE_ID, true),
];

/// Page « Agenda » de la fenêtre de réglages.
const CALENDAR_PAGE: i32 = 4;

impl Controller {
    pub(super) fn open_welcome(self: &Rc<Self>) {
        crate::platform::activate_app();
        if let Some(ui) = self.welcome.borrow().as_ref() {
            let _ = ui.show();
            return;
        }
        let ui = match WelcomeWindow::new() {
            Ok(ui) => ui,
            Err(e) => {
                log::error!("assistant d'accueil : {e}");
                return;
            }
        };
        self.fill_welcome(&ui);

        let weak = Rc::downgrade(self);
        let with = |f: fn(&Rc<Controller>)| {
            let weak = weak.clone();
            move || {
                if let Some(c) = weak.upgrade() {
                    f(&c);
                }
            }
        };
        ui.on_language_changed(with(|c| c.welcome_language()));
        ui.on_install_hooks(with(|c| {
            c.toggle_claude_hooks();
            if let Some(ui) = c.welcome.borrow().as_ref() {
                ui.set_hooks_installed(c.installer.is_installed());
            }
        }));
        ui.on_add_calendar(with(|c| c.open_settings_page(CALENDAR_PAGE)));
        ui.on_open_settings(with(|c| c.open_settings()));
        // Différé : la fenêtre ne se libère pas pendant son propre callback.
        ui.on_finish(|| super::post(|c| c.finish_welcome()));
        // Fermer la fenêtre vaut « Terminer » : les choix affichés sont gardés.
        let close = || super::post(|c| c.finish_welcome());
        ui.window().on_close_requested(move || {
            close();
            slint::CloseRequestResponse::KeepWindowShown
        });

        let _ = ui.show();
        *self.welcome.borrow_mut() = Some(ui);
    }

    fn fill_welcome(&self, ui: &WelcomeWindow) {
        let config = self.config.borrow();
        let on = |id: &str| {
            let default = MODULES.iter().find(|(m, _)| *m == id).is_some_and(|m| m.1);
            config.module_enabled(id, default)
        };
        ui.set_language_index(super::settings::language_index(config.general.language));
        ui.set_claude(on(bw_claude::MODULE_ID));
        ui.set_media(on(bw_media::MODULE_ID));
        ui.set_calendar(on(bw_calendar::MODULE_ID));
        ui.set_notifications(on(bw_notify::MODULE_ID));
        ui.set_weather(on(bw_weather::MODULE_ID));
        ui.set_weather_city(
            bw_weather::WeatherConfig::from_table(config.modules.get(bw_weather::MODULE_ID))
                .map(|w| w.city)
                .unwrap_or_default()
                .into(),
        );
        ui.set_timer(on(bw_timer::MODULE_ID));
        ui.set_tasks(on("tasks"));
        ui.set_mascot(on("mascot"));
        ui.set_network(on(bw_network::MODULE_ID));
        ui.set_system(on(bw_system::MODULE_ID));
        ui.set_claude_detected(bw_claude::install::claude_dir().is_dir());
        ui.set_hooks_installed(self.installer.is_installed());
        ui.set_toggle_hotkey(config.hotkeys.toggle.trim().into());
    }

    /// La langue s'applique tout de suite (textes de l'assistant compris).
    fn welcome_language(&self) {
        let Some(index) = self
            .welcome
            .borrow()
            .as_ref()
            .map(|ui| ui.get_language_index())
        else {
            return;
        };
        let language = match index {
            1 => "en",
            2 => "fr",
            _ => "auto",
        };
        if let Err(e) = self.edit_now(|e| {
            e.set(&["general", "language"], Value::Str(language.into()));
        }) {
            log::warn!("assistant d'accueil : langue non enregistrée : {e}");
        }
    }

    /// Enregistre les modules choisis et ferme l'assistant.
    fn finish_welcome(self: &Rc<Self>) {
        let Some(ui) = self.welcome.borrow_mut().take() else {
            return;
        };
        let _ = ui.hide();
        let city = ui.get_weather_city().trim().to_owned();
        let choices = [
            (bw_claude::MODULE_ID, ui.get_claude()),
            (bw_media::MODULE_ID, ui.get_media()),
            (bw_calendar::MODULE_ID, ui.get_calendar()),
            (bw_notify::MODULE_ID, ui.get_notifications()),
            // Sans ville, la météo ne saurait pas où chercher.
            (bw_weather::MODULE_ID, ui.get_weather() && !city.is_empty()),
            (bw_timer::MODULE_ID, ui.get_timer()),
            ("tasks", ui.get_tasks()),
            ("mascot", ui.get_mascot()),
            (bw_network::MODULE_ID, ui.get_network()),
            (bw_system::MODULE_ID, ui.get_system()),
        ];
        // N'écrire que ce qui change : un config.toml inchangé ne redémarre rien.
        let changes: Vec<(&str, bool)> = {
            let config = self.config.borrow();
            choices
                .into_iter()
                .filter(|(id, enabled)| {
                    let default = MODULES.iter().find(|(m, _)| m == id).is_some_and(|m| m.1);
                    config.module_enabled(id, default) != *enabled
                })
                .collect()
        };
        let weather_city = ui.get_weather() && !city.is_empty();
        if changes.is_empty() && !weather_city {
            return;
        }
        let result = self.edit_now(|e| {
            for (id, enabled) in &changes {
                e.set(&["modules", id, "enabled"], Value::Bool(*enabled));
            }
            if weather_city {
                e.set(
                    &["modules", bw_weather::MODULE_ID, "city"],
                    Value::Str(city),
                );
            }
        });
        match result {
            Ok(_) => log::info!(
                "assistant d'accueil : {} module(s) modifié(s)",
                changes.len()
            ),
            Err(e) => {
                log::error!("assistant d'accueil : {e}");
                self.flash(&e);
            }
        }
    }
}
