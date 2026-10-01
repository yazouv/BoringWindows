//! Hors Windows : la fenêtre s'affiche sans intégration système avancée
//! (placement, zone cliquable, plein écran). macOS a en plus l'icône de barre
//! de menus et pas d'icône dans le Dock.

use std::path::Path;

use bw_config::MonitorChoice;
use slint::BackendSelector;
use slint::winit_030::winit::window::Window;

use super::PlatformEvent;
#[cfg(not(target_os = "macos"))]
use super::TrayCommand;
use crate::geometry::PhysRect;

pub struct SingleInstance;

pub fn single_instance() -> Option<SingleInstance> {
    Some(SingleInstance)
}

#[cfg(not(target_os = "macos"))]
pub fn configure_backend(selector: BackendSelector) -> BackendSelector {
    selector
}

/// Application « accessoire » : pas d'icône dans le Dock ni dans Cmd+Tab.
#[cfg(target_os = "macos")]
pub fn configure_backend(selector: BackendSelector) -> BackendSelector {
    use slint::winit_030::SlintEvent;
    use slint::winit_030::winit::event_loop::EventLoop;
    use slint::winit_030::winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

    let mut builder = EventLoop::<SlintEvent>::with_user_event();
    builder.with_activation_policy(ActivationPolicy::Accessory);
    selector.with_winit_event_loop_builder(builder)
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

    pub fn set_visible(&self, _visible: bool) {}

    pub fn fullscreen_now(&self) -> bool {
        false
    }

    pub fn foreground_on_our_monitor(&self) -> bool {
        true
    }
}

/// Linux : pas d'icône de notification (elle exigerait GTK).
#[cfg(not(target_os = "macos"))]
pub struct Tray;

#[cfg(not(target_os = "macos"))]
impl Tray {
    pub fn new(
        _autostart: bool,
        _claude_hooks_installed: bool,
        _on_command: impl Fn(TrayCommand) + Send + Sync + 'static,
    ) -> anyhow::Result<Self> {
        Ok(Self)
    }

    pub fn set_autostart_checked(&self, _checked: bool) {}

    pub fn set_claude_hooks_installed(&self, _installed: bool) {}

    pub fn retranslate(&self) {}
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

/// Ramener le terminal d'une session au premier plan : Windows uniquement.
pub fn focus_terminal(_ancestors: &[u32], _console_window: Option<i64>) -> bool {
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
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(opener).arg(path).spawn() {
        log::warn!("impossible d'ouvrir {} : {e}", path.display());
    }
}
