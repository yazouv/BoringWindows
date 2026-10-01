//! Mises à jour : vérification au lancement puis toutes les 6 heures (si
//! `general.auto_update`), ou à la demande (menu de l'icône, réglages).
//! La nouvelle version est installée à la place de l'exécutable et prend
//! effet au redémarrage.

use std::rc::Rc;
use std::time::Duration;

use bw_i18n::tr;
use bw_update::Version;
use slint::TimerMode;

use super::{Controller, post};

const FIRST_CHECK: Duration = Duration::from_secs(30);
const EVERY: Duration = Duration::from_secs(6 * 3600);

/// Version de cette build (celle du workspace, gérée par release-please).
pub(super) const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum UpdateState {
    #[default]
    Idle,
    Checking,
    /// Installée, active au prochain lancement.
    Ready(Version),
}

impl Controller {
    /// Au démarrage : nettoyage d'une mise à jour précédente, puis vérifications.
    pub(super) fn start_updates(self: &Rc<Self>) {
        if let Ok(exe) = std::env::current_exe() {
            bw_update::cleanup(&exe);
        }
        self.schedule_update_check(FIRST_CHECK);
    }

    fn schedule_update_check(&self, delay: Duration) {
        self.update_timer.start(TimerMode::SingleShot, delay, || {
            post(|c| {
                if c.config.borrow().general.auto_update {
                    c.check_updates(false);
                }
                c.schedule_update_check(EVERY);
            });
        });
    }

    /// Cherche et installe une mise à jour. `manual` : l'utilisateur l'a
    /// demandé, on lui répond même s'il n'y a rien.
    pub(super) fn check_updates(self: &Rc<Self>, manual: bool) {
        match self.update_state.get() {
            UpdateState::Checking => return,
            UpdateState::Ready(v) => {
                if manual {
                    self.flash(&ready_text(v));
                }
                return;
            }
            UpdateState::Idle => {}
        }
        self.update_state.set(UpdateState::Checking);
        self.sync_update_ui();
        std::thread::spawn(move || {
            let result = check_and_install();
            post(move |c| c.on_update_result(result, manual));
        });
    }

    fn on_update_result(self: &Rc<Self>, result: Result<Option<Version>, String>, manual: bool) {
        let message = match result {
            Ok(Some(v)) => {
                log::info!("mise à jour {v} installée");
                self.update_state.set(UpdateState::Ready(v));
                Some(ready_text(v))
            }
            Ok(None) => {
                self.update_state.set(UpdateState::Idle);
                manual.then(|| {
                    tr!(
                        "BoringWindows is up to date ({CURRENT})",
                        "BoringWindows est à jour ({CURRENT})"
                    )
                })
            }
            Err(e) => {
                log::warn!("mise à jour : {e}");
                self.update_state.set(UpdateState::Idle);
                manual.then(|| tr!("⚠ Update: {e}", "⚠ Mise à jour : {e}"))
            }
        };
        self.sync_update_ui();
        if let Some(text) = message {
            self.flash(&text);
            self.settings_status(&text, text.starts_with('⚠'));
        }
    }

    /// Menu de l'icône : redémarre si une mise à jour attend, sinon vérifie.
    pub(super) fn update_command(self: &Rc<Self>) {
        match self.update_state.get() {
            UpdateState::Ready(_) => self.restart(),
            _ => self.check_updates(true),
        }
    }

    /// Relance l'exécutable (la nouvelle version) et quitte celle-ci.
    pub(super) fn restart(self: &Rc<Self>) {
        self.flush_settings();
        match std::env::current_exe()
            .and_then(|exe| std::process::Command::new(exe).arg("--restarted").spawn())
        {
            Ok(_) => {
                let _ = slint::quit_event_loop();
            }
            Err(e) => self.flash(&tr!(
                "⚠ Restart failed: {e}",
                "⚠ Redémarrage impossible : {e}"
            )),
        }
    }

    /// Menu de l'icône et fenêtre de réglages selon l'état.
    pub(super) fn sync_update_ui(&self) {
        let state = self.update_state.get();
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.set_update_label(&tray_label(state));
        }
        self.sync_settings_update(state);
    }

    pub(super) fn update_tray_label(&self) -> String {
        tray_label(self.update_state.get())
    }
}

fn check_and_install() -> Result<Option<Version>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if bw_update::is_dev_build(&exe) {
        return Err(tr!(
            "disabled for a development build (cargo run)",
            "désactivée pour une version de développement (cargo run)"
        ));
    }
    let Some(update) = bw_update::check(CURRENT).map_err(|e| format!("{e:#}"))? else {
        return Ok(None);
    };
    log::info!(
        "mise à jour {} disponible : {}",
        update.version,
        update.page
    );
    bw_update::install(&update, &exe).map_err(|e| format!("{e:#}"))?;
    Ok(Some(update.version))
}

fn ready_text(v: Version) -> String {
    tr!(
        "BoringWindows {v} installed · restart to use it (icon menu)",
        "BoringWindows {v} installé · redémarre pour l'utiliser (menu de l'icône)"
    )
}

fn tray_label(state: UpdateState) -> String {
    match state {
        UpdateState::Ready(v) => tr!("Restart to update to {v}", "Redémarrer pour passer à {v}"),
        UpdateState::Checking => tr!("Checking for updates…", "Recherche de mise à jour…"),
        UpdateState::Idle => tr!("Check for updates", "Rechercher une mise à jour"),
    }
}
