//! Langue de l'interface. Les textes côté Rust s'écrivent avec [`tr!`] :
//! l'anglais d'abord (comme dans les fichiers Slint), le français ensuite.
//!
//! ```
//! use bw_i18n::{Lang, tr};
//! bw_i18n::set(Lang::Fr);
//! let n = 3;
//! assert_eq!(tr!("in {n} min", "dans {n} min"), "dans 3 min");
//! ```

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Fr,
}

impl Lang {
    /// Code de langue (« en », « fr »).
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
        }
    }

    /// Langue du système : français si la locale commence par « fr »,
    /// anglais sinon.
    pub fn system() -> Lang {
        Self::from_locale(&sys_locale::get_locale().unwrap_or_default())
    }

    fn from_locale(locale: &str) -> Lang {
        if locale.to_ascii_lowercase().starts_with("fr") {
            Lang::Fr
        } else {
            Lang::En
        }
    }
}

const UNSET: u8 = 0;
static CURRENT: AtomicU8 = AtomicU8::new(UNSET);

/// Langue courante ; celle du système tant que [`set`] n'a pas été appelé.
pub fn lang() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::En,
        2 => Lang::Fr,
        _ => {
            let l = Lang::system();
            set(l);
            l
        }
    }
}

/// Change la langue de tout le processus (tous les threads).
pub fn set(lang: Lang) {
    let v = match lang {
        Lang::En => 1,
        Lang::Fr => 2,
    };
    CURRENT.store(v, Ordering::Relaxed);
}

/// Texte traduit : `tr!("anglais", "français", args…)`, avec la syntaxe de
/// `format!` (arguments nommés dans la chaîne acceptés).
#[macro_export]
macro_rules! tr {
    ($en:literal, $fr:literal $(, $arg:expr)* $(,)?) => {
        match $crate::lang() {
            $crate::Lang::Fr => format!($fr $(, $arg)*),
            $crate::Lang::En => format!($en $(, $arg)*),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_detection() {
        assert_eq!(Lang::from_locale("fr-FR"), Lang::Fr);
        assert_eq!(Lang::from_locale("fr_CA.UTF-8"), Lang::Fr);
        assert_eq!(Lang::from_locale("en-US"), Lang::En);
        assert_eq!(Lang::from_locale("de-DE"), Lang::En);
        assert_eq!(Lang::from_locale(""), Lang::En);
    }

    #[test]
    fn switches_at_runtime() {
        let minutes = 4;
        set(Lang::En);
        assert_eq!(tr!("in {minutes} min", "dans {minutes} min"), "in 4 min");
        set(Lang::Fr);
        assert_eq!(tr!("in {} min", "dans {} min", minutes), "dans 4 min");
        assert_eq!(lang().code(), "fr");
    }
}
