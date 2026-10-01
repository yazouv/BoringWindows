//! Thèmes : un jeu de clés `[theme]` prêt à l'emploi. Ceux fournis sont
//! embarqués ; on peut en ajouter dans `themes/<nom>.toml` à côté de
//! config.toml. Les clés écrites dans `[theme]` passent devant le thème.

use std::path::{Path, PathBuf};

use bw_i18n::tr;
use toml::Table;

/// Thèmes fournis : (nom, contenu).
pub const BUILTIN_THEMES: [(&str, &str); 4] = [
    ("default", include_str!("themes/default.toml")),
    ("light", include_str!("themes/light.toml")),
    ("midnight", include_str!("themes/midnight.toml")),
    ("glass", include_str!("themes/glass.toml")),
];

/// Clés qu'un thème fournit : choisir un thème dans les réglages retire ces
/// clés de `[theme]` pour qu'il s'applique entièrement.
pub const THEME_KEYS: [&str; 6] = [
    "background",
    "foreground",
    "accent",
    "border",
    "corner_radius",
    "font",
];

/// Dossier des thèmes personnels, à côté de config.toml.
pub fn themes_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("themes")
}

/// Thèmes disponibles : ceux fournis, puis les fichiers du dossier des thèmes
/// (triés par nom).
pub fn available_themes(config_dir: &Path) -> Vec<String> {
    let mut custom: Vec<String> = std::fs::read_dir(themes_dir(config_dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension()? == "toml")
                .then(|| path.file_stem()?.to_str().map(str::to_owned))
                .flatten()
        })
        .filter(|name| !BUILTIN_THEMES.iter().any(|(b, _)| b == name))
        .collect();
    custom.sort();
    BUILTIN_THEMES
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .chain(custom)
        .collect()
}

/// Remplace la table `theme` de `config` par le thème nommé, complété par
/// les clés que l'utilisateur a écrites lui-même.
pub(crate) fn resolve(config: &mut Table, config_dir: &Path) -> Result<(), String> {
    let user = match config.remove("theme") {
        Some(toml::Value::Table(t)) => t,
        Some(_) => {
            return Err(tr!(
                "[theme] must be a table",
                "[theme] doit être une table"
            ));
        }
        None => Table::new(),
    };
    let name = match user.get("name") {
        Some(toml::Value::String(s)) => s.clone(),
        Some(_) => {
            return Err(tr!(
                "theme.name must be text",
                "theme.name doit être un texte"
            ));
        }
        None => "default".to_owned(),
    };
    let mut merged = preset(&name, config_dir)?;
    merge(&mut merged, user);
    merged.insert("name".into(), toml::Value::String(name));
    config.insert("theme".into(), toml::Value::Table(merged));
    Ok(())
}

fn preset(name: &str, config_dir: &Path) -> Result<Table, String> {
    if let Some((_, text)) = BUILTIN_THEMES.iter().find(|(n, _)| *n == name) {
        return Ok(text.parse().expect("thème fourni invalide"));
    }
    let path = themes_dir(config_dir).join(format!("{name}.toml"));
    let text = std::fs::read_to_string(&path).map_err(|_| {
        tr!(
            "theme \"{name}\" not found (built-in: default, light, midnight, glass; or a file {})",
            "thème « {name} » introuvable (fournis : default, light, midnight, glass ; ou un fichier {})",
            path.display()
        )
    })?;
    let table: Table = text
        .parse()
        .map_err(|e| format!("{} : {e}", path.display()))?;
    if table.contains_key("name") {
        return Err(tr!(
            "{}: a theme file must not contain \"name\"",
            "{} : un fichier de thème ne doit pas contenir « name »",
            path.display()
        ));
    }
    Ok(table)
}

/// Fusion récursive : `over` gagne, les sous-tables sont complétées.
fn merge(base: &mut Table, over: Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, Config};

    #[test]
    fn builtin_themes_are_valid() {
        for (name, _) in BUILTIN_THEMES {
            let c = Config::from_toml_str(&format!("[theme]\nname = \"{name}\""))
                .unwrap_or_else(|e| panic!("{name} : {e}"));
            assert_eq!(c.theme.name, name);
        }
    }

    #[test]
    fn user_keys_override_the_theme() {
        let c = Config::from_toml_str(
            "[theme]\nname = \"light\"\naccent = \"#FF0000\"\n[theme.compact]\nwidth = 200.0\nheight = 30.0",
        )
        .unwrap();
        assert_eq!(c.theme.accent, Color::rgb(0xFF, 0, 0));
        assert_eq!(c.theme.foreground, Color::rgb(0x1C, 0x1C, 0x1E));
        assert_eq!(c.theme.compact.width, 200.0);
        assert_eq!(c.theme.corner_radius, 22.0);
    }

    #[test]
    fn custom_theme_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(themes_dir(dir.path())).unwrap();
        std::fs::write(
            themes_dir(dir.path()).join("rose.toml"),
            "background = \"#FF2D55\"\nfont = \"Segoe UI\"",
        )
        .unwrap();
        std::fs::write(themes_dir(dir.path()).join("notes.txt"), "").unwrap();

        assert_eq!(
            available_themes(dir.path()),
            ["default", "light", "midnight", "glass", "rose"]
        );
        let c = Config::parse("[theme]\nname = \"rose\"", dir.path()).unwrap();
        assert_eq!(c.theme.background, Color::rgb(0xFF, 0x2D, 0x55));
        assert_eq!(c.theme.font, "Segoe UI");
        // Ce que le thème ne précise pas vient des valeurs par défaut.
        assert_eq!(c.theme.accent, Color::rgb(0xFF, 0x8A, 0x3D));

        assert!(Config::parse("[theme]\nname = \"absent\"", dir.path()).is_err());
        std::fs::write(themes_dir(dir.path()).join("faux.toml"), "colour = 1").unwrap();
        assert!(Config::parse("[theme]\nname = \"faux\"", dir.path()).is_err());
    }
}
