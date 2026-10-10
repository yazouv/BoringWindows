//! Raccourcis clavier globaux (`[hotkeys]` de config.toml), enregistrés
//! auprès du système : `RegisterHotKey` sous Windows, Carbon sous macOS, X11
//! sous Linux.

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use bw_i18n::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    Toggle,
    PlayPause,
    Next,
    Previous,
    DoNotDisturb,
}

impl HotkeyAction {
    fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "toggle" => Self::Toggle,
            "play_pause" => Self::PlayPause,
            "next_track" => Self::Next,
            "previous_track" => Self::Previous,
            "do_not_disturb" => Self::DoNotDisturb,
            _ => return None,
        })
    }
}

/// Lit un raccourci (« Ctrl+Alt+B ») ; l'erreur est un message pour l'île.
pub fn parse(text: &str) -> Result<HotKey, String> {
    let hotkey: HotKey = text.parse().map_err(|_| {
        tr!(
            "\"{text}\" is not a valid shortcut (example: Ctrl+Alt+B)",
            "« {text} » n'est pas un raccourci valide (exemple : Ctrl+Alt+B)"
        )
    })?;
    // Une touche seule (lettre, chiffre…) se taperait dans toutes les applications.
    let lone_function_key = matches!(
        hotkey.key,
        global_hotkey::hotkey::Code::F13
            | global_hotkey::hotkey::Code::F14
            | global_hotkey::hotkey::Code::F15
            | global_hotkey::hotkey::Code::F16
            | global_hotkey::hotkey::Code::F17
            | global_hotkey::hotkey::Code::F18
            | global_hotkey::hotkey::Code::F19
            | global_hotkey::hotkey::Code::F20
            | global_hotkey::hotkey::Code::F21
            | global_hotkey::hotkey::Code::F22
            | global_hotkey::hotkey::Code::F23
            | global_hotkey::hotkey::Code::F24
            | global_hotkey::hotkey::Code::Pause
            | global_hotkey::hotkey::Code::ScrollLock
    );
    if hotkey.mods.is_empty() && !lone_function_key {
        return Err(tr!(
            "\"{text}\" needs a modifier (Ctrl, Alt, Shift or Super)",
            "« {text} » doit avoir un modificateur (Ctrl, Alt, Shift ou Super)"
        ));
    }
    Ok(hotkey)
}

/// Raccourcis de la config, et erreurs (syntaxe, doublons) à signaler.
pub fn bindings(config: &bw_config::Hotkeys) -> (Vec<(HotKey, HotkeyAction)>, Vec<String>) {
    let mut found: Vec<(HotKey, HotkeyAction)> = Vec::new();
    let mut errors = Vec::new();
    for (key, text) in config.entries() {
        let text = text.trim();
        let Some(action) = HotkeyAction::from_key(key) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        match parse(text) {
            Ok(hotkey) if found.iter().any(|(h, _)| h.id() == hotkey.id()) => {
                errors.push(tr!(
                    "hotkeys.{key}: \"{text}\" is already used by another action",
                    "hotkeys.{key} : « {text} » sert déjà à une autre action"
                ));
            }
            Ok(hotkey) => found.push((hotkey, action)),
            Err(e) => errors.push(format!("hotkeys.{key} : {e}")),
        }
    }
    (found, errors)
}

/// Raccourcis enregistrés auprès du système.
pub struct Hotkeys {
    manager: GlobalHotKeyManager,
    /// Raccourcis valides demandés par la config, et ceux que le système a
    /// refusés (déjà pris).
    wanted: Vec<(HotKey, HotkeyAction)>,
    refused: Vec<String>,
    registered: Vec<(HotKey, HotkeyAction)>,
}

impl Hotkeys {
    /// À créer sur le thread UI (Windows : la fenêtre qui reçoit `WM_HOTKEY`
    /// vit sur ce thread). `on_pressed` est appelé depuis n'importe quel thread.
    pub fn new(on_pressed: impl Fn(u32) + Send + Sync + 'static) -> anyhow::Result<Self> {
        let manager = GlobalHotKeyManager::new()?;
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state == HotKeyState::Pressed {
                on_pressed(event.id);
            }
        }));
        Ok(Self {
            manager,
            wanted: Vec::new(),
            refused: Vec::new(),
            registered: Vec::new(),
        })
    }

    /// Remplace les raccourcis enregistrés ; renvoie les erreurs à afficher.
    pub fn apply(&mut self, config: &bw_config::Hotkeys) -> Vec<String> {
        let (wanted, mut errors) = bindings(config);
        if wanted == self.wanted {
            errors.extend(self.refused.iter().cloned());
            return errors;
        }
        for (hotkey, _) in self.registered.drain(..) {
            if let Err(e) = self.manager.unregister(hotkey) {
                log::warn!("raccourci {hotkey} non retiré : {e}");
            }
        }
        self.refused.clear();
        for &(hotkey, action) in &wanted {
            match self.manager.register(hotkey) {
                Ok(()) => {
                    log::info!("raccourci {hotkey} : {action:?}");
                    self.registered.push((hotkey, action));
                }
                Err(e) => {
                    log::warn!("raccourci {hotkey} refusé : {e}");
                    self.refused.push(tr!(
                        "shortcut {} is already taken by another application",
                        "le raccourci {} est déjà pris par une autre application",
                        hotkey
                    ));
                }
            }
        }
        self.wanted = wanted;
        errors.extend(self.refused.iter().cloned());
        errors
    }

    pub fn action(&self, id: u32) -> Option<HotkeyAction> {
        self.registered
            .iter()
            .find(|(h, _)| h.id() == id)
            .map(|(_, a)| *a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_rejects() {
        assert!(parse("Ctrl+Alt+B").is_ok());
        assert!(parse("ctrl + shift + F9").is_ok());
        assert!(parse("Super+Space").is_ok());
        assert!(parse("F13").is_ok());
        assert!(parse("B").is_err());
        assert!(parse("Ctrl+").is_err());
        assert!(parse("Ctrl+Alt+Truc").is_err());
    }

    #[test]
    fn bindings_report_errors_and_duplicates() {
        let config = bw_config::Hotkeys {
            toggle: "Ctrl+Alt+B".into(),
            play_pause: "ctrl+alt+b".into(),
            next_track: "Ctrl+Alt+Right".into(),
            previous_track: "nope".into(),
            do_not_disturb: String::new(),
        };
        let (found, errors) = bindings(&config);
        assert_eq!(
            found.iter().map(|(_, a)| *a).collect::<Vec<_>>(),
            [HotkeyAction::Toggle, HotkeyAction::Next]
        );
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(bindings(&bw_config::Hotkeys::default()).1.is_empty());
    }
}
