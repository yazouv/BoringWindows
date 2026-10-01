//! Intégration système. L'implémentation réelle est Win32 ; ailleurs, des
//! substituts sans effet permettent de travailler l'UI et la logique.

#[cfg(not(windows))]
mod fallback;
#[cfg(windows)]
mod win32;

#[cfg(not(windows))]
pub use fallback::*;
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
}

/// Commandes du menu de la zone de notification.
// Construits uniquement par l'implémentation Win32.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    OpenConfig,
    ReloadConfig,
    Autostart(bool),
    Pause(bool),
    Quit,
}
