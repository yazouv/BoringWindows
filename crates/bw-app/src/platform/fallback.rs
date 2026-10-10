//! Linux et autres : la fenêtre s'affiche sans intégration système avancée
//! (placement, zone cliquable, plein écran, icône de notification).

use std::path::Path;

use bw_config::MonitorChoice;
use slint::BackendSelector;
use slint::winit_030::winit::window::Window;

use super::{PlatformEvent, SystemLook, TrayCommand};
use crate::geometry::{PhysRect, RoundRect};

pub struct SingleInstance;

pub fn single_instance() -> Option<SingleInstance> {
    Some(SingleInstance)
}

pub fn configure_backend(selector: BackendSelector) -> BackendSelector {
    selector
}

pub fn initial_position(_: MonitorChoice, _: (f32, f32)) -> Option<slint::PhysicalPosition> {
    None
}

pub struct Platform;

impl Platform {
    pub fn attach(
        _window: &Window,
        _on_event: impl Fn(PlatformEvent) + 'static,
    ) -> anyhow::Result<Self> {
        Ok(Self)
    }

    pub fn place(&self, _window: &slint::Window, _choice: MonitorChoice, _size: (f32, f32)) {}

    pub fn set_hit_region(&self, _rect: PhysRect) {}

    pub fn set_round_region(&self, _shape: RoundRect) {}

    pub fn set_blur(&self, _on: bool) {}

    pub fn set_capture_excluded(&self, _excluded: bool) {}

    pub fn watch_user_return(&self) {}

    pub fn set_visible(&self, _visible: bool) {}

    pub fn fullscreen_now(&self) -> bool {
        false
    }

    pub fn foreground_on_our_monitor(&self) -> bool {
        true
    }

    pub fn foreground_is_capture_tool(&self) -> bool {
        false
    }

    pub fn top_taskbar_height(&self) -> Option<f32> {
        None
    }
}

/// Linux : pas d'icône de notification (elle exigerait GTK).
pub struct Tray;

impl Tray {
    pub fn new(
        _autostart: bool,
        _claude_hooks_installed: bool,
        _update_label: &str,
        _on_command: impl Fn(TrayCommand) + Send + Sync + 'static,
    ) -> anyhow::Result<Self> {
        Ok(Self)
    }

    pub fn set_autostart_checked(&self, _checked: bool) {}

    pub fn set_claude_hooks_installed(&self, _installed: bool) {}

    pub fn retranslate(&self) {}

    pub fn set_update_label(&self, _label: &str) {}
}

pub fn autostart_enabled() -> bool {
    false
}

pub fn set_autostart(_enabled: bool) -> anyhow::Result<()> {
    anyhow::bail!(bw_i18n::tr!(
        "starting with the system isn't available on this platform yet",
        "démarrage automatique pas encore disponible sur ce système"
    ))
}

/// Pas de boîte de dialogue native ici : on accepte et on journalise.
pub fn confirm(title: &str, text: &str) -> bool {
    log::info!("{title} : {text}");
    true
}

pub fn activate_app() {}

/// Ramener le terminal d'une session au premier plan : Windows uniquement.
pub fn focus_terminal(_ancestors: &[u32], _console_window: Option<i64>) -> bool {
    false
}

/// Rejouer une notification : Windows uniquement.
pub fn open_notification(_lines: &[String]) -> bool {
    false
}

/// Ouvrir l'application d'une notification : Windows uniquement.
pub fn open_app(_app_id: &str) -> bool {
    false
}

pub fn attach_parent_console() {}

pub fn creating_island<R>(f: impl FnOnce() -> R) -> R {
    f()
}

/// Pas de sélecteur de fichier natif ici : le chemin se tape à la main.
pub fn pick_ics_file() -> Option<std::path::PathBuf> {
    None
}

pub fn alert_sound() {}

/// Ouvre un fichier avec l'application associée.
pub fn open_path(path: &Path) {
    if let Err(e) = std::process::Command::new("xdg-open").arg(path).spawn() {
        log::warn!("impossible d'ouvrir {} : {e}", path.display());
    }
}

/// Reprise d'une conversation Claude Code : Windows et macOS seulement.
pub fn resume_claude_session(_cwd: &Path, _session_id: &str) -> bool {
    log::warn!("rouvrir une conversation n'est géré que sous Windows et macOS");
    false
}

/// Hors Windows : thème sombre, accent du thème.
pub fn system_look() -> SystemLook {
    SystemLook::default()
}

pub struct LookWatcher;

pub fn watch_system_look(_on_change: impl Fn() + Send + 'static) -> Option<LookWatcher> {
    None
}

/// Hors Windows : jamais inactif (la mascotte ne s'endort pas).
pub fn idle_for() -> std::time::Duration {
    std::time::Duration::ZERO
}
