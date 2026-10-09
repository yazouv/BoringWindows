//! Intégration système. L'implémentation complète est Win32 ; macOS a l'icône
//! de barre de menus, et ailleurs des substituts permettent de travailler l'UI.

#[cfg(not(windows))]
mod fallback;
#[cfg(any(windows, target_os = "macos"))]
mod tray;
#[cfg(windows)]
mod win32;

#[cfg(not(windows))]
pub use fallback::*;
#[cfg(any(windows, target_os = "macos"))]
pub use tray::Tray;
#[cfg(windows)]
pub use win32::*;

/// Événements système remontés à l'application (sur le thread UI).
// Construits uniquement par l'implémentation Win32.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformEvent {
    /// Une application passe en plein écran (ou en sort) sur l'écran de l'île.
    Fullscreen(bool),
    /// Écrans branchés/débranchés ou résolution modifiée.
    DisplayChanged,
    /// Une autre fenêtre passe au premier plan.
    Foreground,
    /// La barre des tâches a bougé, changé de taille, ou l'Explorateur a redémarré.
    TaskbarChanged,
}

/// Commandes du menu de la zone de notification.
// Construits uniquement par l'implémentation Win32.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// Ouvrir la fenêtre de réglages.
    Settings,
    OpenConfig,
    ReloadConfig,
    Autostart(bool),
    Pause(bool),
    /// Installer ou retirer les hooks Claude Code.
    ClaudeHooks,
    /// Rechercher une mise à jour, ou redémarrer si elle est installée.
    Update,
    Quit,
}
