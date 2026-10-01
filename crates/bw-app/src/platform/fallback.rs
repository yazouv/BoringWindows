//! Substituts hors Windows : la fenêtre s'affiche, sans intégration système.

use std::path::Path;

use bw_config::MonitorChoice;
use slint::winit_030::winit::window::{Window, WindowAttributes};

use super::{PlatformEvent, TrayCommand};
use crate::geometry::PhysRect;

pub struct SingleInstance;

pub fn single_instance() -> Option<SingleInstance> {
    Some(SingleInstance)
}

pub fn window_attributes(attrs: WindowAttributes) -> WindowAttributes {
    attrs
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

pub struct Tray;

impl Tray {
    pub fn new(
        _autostart: bool,
        _on_command: impl Fn(TrayCommand) + Send + Sync + 'static,
    ) -> anyhow::Result<Self> {
        Ok(Self)
    }

    pub fn set_autostart_checked(&self, _checked: bool) {}
}

pub fn autostart_enabled() -> bool {
    false
}

pub fn set_autostart(_enabled: bool) -> anyhow::Result<()> {
    anyhow::bail!("démarrage automatique disponible uniquement sous Windows")
}

pub fn open_path(path: &Path) {
    log::info!("ouvrir {}", path.display());
}
