//! Intégration système : Win32 (complète), macOS (panneau au-dessus de la
//! barre de menus), et ailleurs des substituts pour travailler l'UI.

#[cfg(not(any(windows, target_os = "macos")))]
mod fallback;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(windows, target_os = "macos"))]
mod tray;
#[cfg(windows)]
mod win32;

#[cfg(not(any(windows, target_os = "macos")))]
pub use fallback::*;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(any(windows, target_os = "macos"))]
pub use tray::Tray;
#[cfg(windows)]
pub use win32::*;

/// Apparence de Windows (mode des applications, couleur d'accent).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SystemLook {
    pub light: bool,
    pub accent: Option<[u8; 3]>,
}

/// Événements système remontés à l'application (sur le thread UI).
// Construits par les implémentations Win32 et macOS (en partie).
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
    /// Clavier ou souris utilisés après `watch_user_return` (l'utilisateur
    /// est revenu).
    UserReturned,
    /// La souris a quitté la pilule sans que la fenêtre le voie (macOS : la
    /// fenêtre ignore la souris hors de la pilule).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    PointerLeft,
}

/// Commandes du menu de la zone de notification.
// Construits seulement par l'icône de notification (Windows, macOS).
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
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
