//! Configuration de BoringWindows (`%APPDATA%\BoringWindows\config.toml`).
//!
//! Le fichier est créé avec des valeurs commentées au premier lancement, validé
//! au chargement, et surveillé pour être rechargé à chaud.

mod color;
mod edit;
mod theme;
mod watch;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use bw_i18n::tr;

pub use color::Color;
pub use edit::{ConfigEditor, Value};
pub use theme::{BUILTIN_THEMES, THEME_KEYS, available_themes, themes_dir};
pub use watch::{ConfigWatcher, watch};

/// Modèle du fichier créé au premier lancement. Il doit rester équivalent à
/// `Config::default()` (vérifié par les tests).
pub const DEFAULT_TOML: &str = include_str!("default.toml");

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{}", tr!("cannot access {} : {}", "impossible d'accéder à {} : {}", path.display(), source))]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{}", tr!("invalid config.toml: {}", "config.toml invalide : {}", .0))]
    Parse(#[from] toml::de::Error),
    #[error("{}", tr!("invalid config.toml:\n  - {}", "config.toml invalide :\n  - {}", .0.join("\n  - ")))]
    Invalid(Vec<String>),
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub general: General,
    pub theme: Theme,
    pub layout: Layout,
    /// Sections `[modules.<nom>]`, interprétées par chaque module.
    pub modules: BTreeMap<String, toml::Table>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    pub monitor: MonitorChoice,
    pub hide_in_fullscreen: bool,
    pub open_on: OpenOn,
    pub collapse_delay_ms: u32,
    pub language: Language,
    /// Installer les mises à jour (releases GitHub) automatiquement.
    pub auto_update: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            monitor: MonitorChoice::Primary,
            hide_in_fullscreen: true,
            open_on: OpenOn::Hover,
            collapse_delay_ms: 350,
            language: Language::Auto,
            auto_update: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MonitorChoice {
    Primary,
    Cursor,
}

/// Langue de l'interface ; `Auto` suit celle de Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Auto,
    En,
    Fr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenOn {
    Hover,
    Click,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    /// Thème de base (`default`, `light`… ou `themes/<nom>.toml`).
    pub name: String,
    pub background: Color,
    pub foreground: Color,
    pub accent: Color,
    /// Contour de l'île (transparent : pas de contour).
    pub border: Color,
    /// Police ; vide : celle du système.
    pub font: String,
    pub corner_radius: f32,
    pub animation_ms: u32,
    pub top_offset: f32,
    pub compact: Size,
    pub attention: Size,
    pub expanded: Size,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "default".into(),
            background: Color::rgb(0x00, 0x00, 0x00),
            foreground: Color::rgb(0xFF, 0xFF, 0xFF),
            accent: Color::rgb(0xFF, 0x8A, 0x3D),
            border: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
            font: String::new(),
            corner_radius: 22.0,
            animation_ms: 240,
            top_offset: 0.0,
            compact: Size::new(190.0, 32.0),
            attention: Size::new(300.0, 36.0),
            expanded: Size::new(520.0, 170.0),
        }
    }
}

/// Taille en pixels logiques (avant mise à l'échelle DPI).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Layout {
    /// Ordre de priorité des modules en mode compact.
    pub compact: Vec<String>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            compact: vec!["claude".into(), "media".into(), "calendar".into()],
        }
    }
}

impl Config {
    /// Lit une config ; les thèmes personnels sont cherchés à côté de
    /// config.toml, dans le dossier habituel.
    pub fn from_toml_str(s: &str) -> Result<Self, ConfigError> {
        Self::parse(s, &config_dir())
    }

    /// Lit une config dont les thèmes personnels sont dans
    /// `config_dir/themes`.
    pub fn parse(s: &str, config_dir: &Path) -> Result<Self, ConfigError> {
        // Premier passage sur le texte brut : messages d'erreur avec la ligne.
        toml::from_str::<Config>(s)?;
        let mut table: toml::Table = s.parse()?;
        theme::resolve(&mut table, config_dir).map_err(|e| ConfigError::Invalid(vec![e]))?;
        let config: Config = toml::Value::Table(table).try_into()?;
        config.validate()?;
        Ok(config)
    }

    /// Charge la config ; si le fichier n'existe pas, il est créé avec le modèle
    /// par défaut.
    pub fn load_or_create(path: &Path) -> Result<Self, ConfigError> {
        let io_err = |source| ConfigError::Io {
            path: path.to_owned(),
            source,
        };
        match std::fs::read_to_string(path) {
            Ok(s) => Self::parse(&s, path.parent().unwrap_or(Path::new("."))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(io_err)?;
                }
                std::fs::write(path, DEFAULT_TOML).map_err(io_err)?;
                log::info!("config par défaut créée : {}", path.display());
                Ok(Self::default())
            }
            Err(e) => Err(io_err(e)),
        }
    }

    /// Un module est actif selon `enabled` dans sa section `[modules.<id>]`,
    /// ou selon `default` si la section ou la clé est absente.
    pub fn module_enabled(&self, id: &str, default: bool) -> bool {
        self.modules
            .get(id)
            .and_then(|m| m.get("enabled"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(default)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let mut errors = Vec::new();
        let t = &self.theme;

        for (name, size) in [
            ("theme.compact", t.compact),
            ("theme.attention", t.attention),
            ("theme.expanded", t.expanded),
        ] {
            if !(size.width >= 16.0 && size.height >= 8.0)
                || size.width > 4000.0
                || size.height > 2000.0
            {
                errors.push(tr!(
                    "{name}: size out of range ({}x{})",
                    "{name} : taille hors limites ({}x{})",
                    size.width,
                    size.height
                ));
            }
        }
        if t.expanded.width < t.attention.width.max(t.compact.width)
            || t.expanded.height < t.attention.height.max(t.compact.height)
        {
            errors.push(tr!(
                "theme.expanded must be at least as large as compact and attention",
                "theme.expanded doit être au moins aussi grand que compact et attention"
            ));
        }
        if !(0.0..=500.0).contains(&t.corner_radius) {
            errors.push(tr!(
                "theme.corner_radius must be between 0 and 500",
                "theme.corner_radius doit être entre 0 et 500"
            ));
        }
        if t.animation_ms > 2000 {
            errors.push(tr!(
                "theme.animation_ms must be ≤ 2000",
                "theme.animation_ms doit être ≤ 2000"
            ));
        }
        if !(0.0..=500.0).contains(&t.top_offset) {
            errors.push(tr!(
                "theme.top_offset must be between 0 and 500",
                "theme.top_offset doit être entre 0 et 500"
            ));
        }
        if self.general.collapse_delay_ms > 10_000 {
            errors.push(tr!(
                "general.collapse_delay_ms must be ≤ 10000",
                "general.collapse_delay_ms doit être ≤ 10000"
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Invalid(errors))
        }
    }
}

/// `%APPDATA%\BoringWindows` sous Windows (équivalent XDG ailleurs).
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("BoringWindows")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_template_matches_default_config() {
        let mut parsed = Config::from_toml_str(DEFAULT_TOML).unwrap();
        // Le modèle documente le module demo, désactivé.
        assert!(!parsed.module_enabled("demo", true));
        assert!(parsed.module_enabled("claude", false));
        parsed.modules.clear();
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn empty_file_gives_defaults() {
        assert_eq!(Config::from_toml_str("").unwrap(), Config::default());
    }

    #[test]
    fn partial_sections_keep_other_defaults() {
        let c = Config::from_toml_str(
            r##"
            [general]
            open_on = "click"
            [theme]
            accent = "#00FF0080"
            [theme.expanded]
            width = 600
            height = 200
            "##,
        )
        .unwrap();
        assert_eq!(c.general.open_on, OpenOn::Click);
        assert!(c.general.hide_in_fullscreen);
        assert_eq!(
            c.theme.accent,
            Color {
                r: 0,
                g: 255,
                b: 0,
                a: 128
            }
        );
        assert_eq!(c.theme.expanded, Size::new(600.0, 200.0));
        assert_eq!(c.theme.compact, Theme::default().compact);
    }

    #[test]
    fn typos_are_rejected() {
        let err = Config::from_toml_str("[general]\nhide_in_fulscreen = false").unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)), "{err}");
    }

    #[test]
    fn bad_values_are_all_reported() {
        let err = Config::from_toml_str(
            "[theme]\nanimation_ms = 9000\ncorner_radius = -1\n[theme.expanded]\nwidth = 50\nheight = 20",
        )
        .unwrap_err();
        let ConfigError::Invalid(errors) = err else {
            panic!("attendu Invalid, obtenu {err}");
        };
        assert_eq!(errors.len(), 3, "{errors:?}");
    }

    #[test]
    fn module_sections() {
        let c = Config::from_toml_str(
            "[modules.a]\n[modules.b]\nenabled = false\n[modules.c]\nenabled = true\nfoo = 1",
        )
        .unwrap();
        assert!(c.module_enabled("a", true));
        assert!(!c.module_enabled("a", false));
        assert!(!c.module_enabled("b", true));
        assert!(c.module_enabled("c", false));
        assert!(c.module_enabled("missing", true));
        assert!(!c.module_enabled("missing", false));
    }

    #[test]
    fn load_or_create_writes_template_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("config.toml");
        assert_eq!(Config::load_or_create(&path).unwrap(), Config::default());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), DEFAULT_TOML);

        std::fs::write(&path, "[general]\nopen_on = \"click\"").unwrap();
        assert_eq!(
            Config::load_or_create(&path).unwrap().general.open_on,
            OpenOn::Click
        );
    }
}
