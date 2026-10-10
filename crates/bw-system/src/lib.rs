//! Processeur et mémoire, affichés dans l'en-tête de l'île ouverte.
//!
//! Mesurer l'usage du processeur demande deux relevés espacés : le module ne
//! mesure que pendant que l'île est ouverte, sauf si une alerte est réglée
//! (relevé toutes les quelques secondes, île fermée comprise).

mod config;
mod module;
mod watch;

pub use config::SystemConfig;
pub use module::{MODULE_ID, SystemModule};
pub use watch::Watch;

/// Ce que publie le module.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemSnapshot {
    /// Usage du processeur, tous cœurs confondus (0 à 100).
    pub cpu: f32,
    /// Mémoire utilisée et totale (octets).
    pub memory_used: u64,
    pub memory_total: u64,
}

impl SystemSnapshot {
    /// Mémoire utilisée (0 à 100).
    pub fn ram_percent(&self) -> f32 {
        if self.memory_total == 0 {
            return 0.0;
        }
        (self.memory_used as f64 / self.memory_total as f64 * 100.0) as f32
    }

    /// « 12% » : texte court de l'en-tête.
    pub fn cpu_text(&self) -> String {
        percent(self.cpu)
    }

    pub fn ram_text(&self) -> String {
        percent(self.ram_percent())
    }

    /// « Memory 8.4 / 16.0 GB » : détail montré au survol, à la place de la
    /// date (le processeur est déjà sur son compteur).
    pub fn detail(&self) -> String {
        let used = gigabytes(self.memory_used);
        let total = gigabytes(self.memory_total);
        bw_i18n::tr!("Memory {used} / {total} GB", "Mémoire {used} / {total} Go")
    }
}

fn percent(value: f32) -> String {
    format!("{}%", value.clamp(0.0, 100.0).round() as u32)
}

/// Gigaoctets (binaires, comme le Gestionnaire des tâches), une décimale
/// sous 100.
fn gigabytes(bytes: u64) -> String {
    let gb = bytes as f64 / (1u64 << 30) as f64;
    let text = if gb < 100.0 {
        format!("{gb:.1}")
    } else {
        format!("{gb:.0}")
    };
    match bw_i18n::lang() {
        bw_i18n::Lang::Fr => text.replace('.', ","),
        bw_i18n::Lang::En => text,
    }
}

/// Nom lisible d'un processus (« chrome.exe » → « chrome »).
pub fn process_label(name: &str) -> &str {
    name.strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts() {
        let s = SystemSnapshot {
            cpu: 12.4,
            memory_used: 8 << 30,
            memory_total: 16 << 30,
        };
        assert_eq!(s.cpu_text(), "12%");
        assert_eq!(s.ram_text(), "50%");
        bw_i18n::set(bw_i18n::Lang::Fr);
        assert_eq!(s.detail(), "Mémoire 8,0 / 16,0 Go");
        let empty = SystemSnapshot {
            cpu: 140.0,
            memory_used: 0,
            memory_total: 0,
        };
        assert_eq!(empty.ram_percent(), 0.0);
        assert_eq!(empty.cpu_text(), "100%");
        assert_eq!(process_label("chrome.exe"), "chrome");
        assert_eq!(process_label("firefox"), "firefox");
    }
}
