//! Icône de la zone de notification et son menu.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::platform::TrayCommand;

pub struct Tray {
    _icon: TrayIcon,
    autostart: CheckMenuItem,
    autostart_state: Arc<AtomicBool>,
}

impl Tray {
    /// `on_command` est appelé depuis le gestionnaire d'événements de `muda` :
    /// il doit renvoyer le travail vers le thread UI.
    pub fn new(
        autostart: bool,
        on_command: impl Fn(TrayCommand) + Send + Sync + 'static,
    ) -> anyhow::Result<Self> {
        let title = MenuItem::new("BoringWindows", false, None);
        let open = MenuItem::new("Ouvrir la configuration", true, None);
        let reload = MenuItem::new("Recharger la configuration", true, None);
        let autostart_item = CheckMenuItem::new("Lancer au démarrage", true, autostart, None);
        let pause = CheckMenuItem::new("Masquer l'île", true, false, None);
        let quit = MenuItem::new("Quitter", true, None);

        let menu = Menu::new();
        menu.append_items(&[
            &title,
            &PredefinedMenuItem::separator(),
            &open,
            &reload,
            &PredefinedMenuItem::separator(),
            &autostart_item,
            &pause,
            &PredefinedMenuItem::separator(),
            &quit,
        ])?;

        // Les cases à cocher basculent toutes seules au clic : on suit leur
        // état ici, les éléments de menu n'étant pas utilisables hors du thread UI.
        let autostart_state = Arc::new(AtomicBool::new(autostart));
        let pause_state = AtomicBool::new(false);
        let ids = (
            open.id().clone(),
            reload.id().clone(),
            autostart_item.id().clone(),
            pause.id().clone(),
            quit.id().clone(),
        );
        let autostart_flag = autostart_state.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let (open, reload, autostart, pause, quit) = &ids;
            let toggle = |flag: &AtomicBool| !flag.fetch_xor(true, Ordering::Relaxed);
            let command = match &event.id {
                id if id == open => TrayCommand::OpenConfig,
                id if id == reload => TrayCommand::ReloadConfig,
                id if id == autostart => TrayCommand::Autostart(toggle(&autostart_flag)),
                id if id == pause => TrayCommand::Pause(toggle(&pause_state)),
                id if id == quit => TrayCommand::Quit,
                _ => return,
            };
            on_command(command);
        }));

        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("BoringWindows")
            .with_icon(pill_icon()?)
            .build()?;

        Ok(Self {
            _icon: icon,
            autostart: autostart_item,
            autostart_state,
        })
    }

    /// Corrige la case si l'écriture dans le registre a échoué.
    pub fn set_autostart_checked(&self, checked: bool) {
        self.autostart.set_checked(checked);
        self.autostart_state.store(checked, Ordering::Relaxed);
    }
}

/// Icône générée : une pilule blanche avec un point orange, pas de fichier à embarquer.
fn pill_icon() -> anyhow::Result<Icon> {
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    let (cx, cy, half_w, r) = (15.5_f32, 15.5_f32, 13.0_f32, 7.0_f32);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (px, py) = (x as f32, y as f32);
            // Distance signée à une pilule horizontale (segment épaissi).
            let dx = ((px - cx).abs() - (half_w - r)).max(0.0);
            let dy = py - cy;
            let pill = (dx * dx + dy * dy).sqrt() - r;
            let dot = ((px - (cx + half_w - r)).powi(2) + dy * dy).sqrt() - 3.5;
            let alpha = |d: f32| ((0.5 - d).clamp(0.0, 1.0) * 255.0) as u8;
            if dot < 0.5 {
                rgba.extend_from_slice(&[0xFF, 0x8A, 0x3D, alpha(dot)]);
            } else {
                rgba.extend_from_slice(&[0xFF, 0xFF, 0xFF, alpha(pill)]);
            }
        }
    }
    Ok(Icon::from_rgba(rgba, SIZE, SIZE)?)
}
