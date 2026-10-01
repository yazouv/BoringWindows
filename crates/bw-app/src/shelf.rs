//! Étagère : des fichiers déposés sur l'île pour les garder sous la main.
//! On garde des références (chemins), jamais de copies ; la liste survit au
//! redémarrage (`shelf.txt`, un chemin par ligne, à côté de config.toml).

use std::path::{Path, PathBuf};

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.shelf]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShelfConfig {
    pub enabled: bool,
    /// Nombre maximal de fichiers gardés (les plus anciens sortent).
    pub max: usize,
}

impl Default for ShelfConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max: 8,
        }
    }
}

impl ShelfConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (1..=30).contains(&config.max),
            tr!(
                "modules.shelf.max must be between 1 and 30",
                "modules.shelf.max doit être entre 1 et 30"
            )
        );
        Ok(config)
    }
}

#[derive(Debug, Default)]
pub struct Shelf {
    items: Vec<PathBuf>,
}

impl Shelf {
    /// Lit `shelf.txt` ; les fichiers disparus sont écartés.
    pub fn load(file: &Path) -> Self {
        let items = std::fs::read_to_string(file)
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect();
        Self { items }
    }

    pub fn save(&self, file: &Path) {
        let text: String = self
            .items
            .iter()
            .map(|p| format!("{}\n", p.display()))
            .collect();
        if let Err(e) = std::fs::write(file, text) {
            log::warn!("étagère : {e}");
        }
    }

    pub fn items(&self) -> &[PathBuf] {
        &self.items
    }

    /// Ajoute en tête (un fichier déjà présent remonte) ; garde au plus `max`.
    pub fn add(&mut self, paths: impl IntoIterator<Item = PathBuf>, max: usize) {
        for path in paths {
            self.items.retain(|p| *p != path);
            self.items.insert(0, path);
        }
        self.items.truncate(max);
    }

    pub fn remove(&mut self, index: usize) -> Option<PathBuf> {
        (index < self.items.len()).then(|| self.items.remove(index))
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Retire les fichiers qui n'existent plus ; vrai si la liste a changé.
    pub fn prune(&mut self) -> bool {
        let before = self.items.len();
        self.items.retain(|p| p.exists());
        self.items.len() != before
    }

    pub fn truncate(&mut self, max: usize) {
        self.items.truncate(max);
    }
}

/// Nom affiché d'un chemin (dernier élément).
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bw-shelf-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn add_dedupes_orders_and_truncates() {
        let mut s = Shelf::default();
        let p = |n: &str| PathBuf::from(n);
        s.add([p("a"), p("b")], 3);
        assert_eq!(s.items(), [p("b"), p("a")]);
        s.add([p("a")], 3);
        assert_eq!(s.items(), [p("a"), p("b")]);
        s.add([p("c"), p("d")], 3);
        assert_eq!(s.items(), [p("d"), p("c"), p("a")]);
        assert_eq!(s.remove(1), Some(p("c")));
        assert_eq!(s.remove(9), None);
        s.clear();
        assert!(s.items().is_empty());
    }

    #[test]
    fn persists_and_drops_missing_files() {
        let dir = temp_dir("persist");
        let keep = dir.join("garde.txt");
        let gone = dir.join("disparu.txt");
        std::fs::write(&keep, "x").unwrap();
        std::fs::write(&gone, "x").unwrap();
        let file = dir.join("shelf.txt");

        let mut s = Shelf::default();
        s.add([gone.clone(), keep.clone()], 8);
        s.save(&file);
        std::fs::remove_file(&gone).unwrap();

        let loaded = Shelf::load(&file);
        assert_eq!(loaded.items(), std::slice::from_ref(&keep));

        let mut s2 = Shelf::default();
        s2.add([keep.clone(), gone], 8);
        assert!(s2.prune());
        assert_eq!(s2.items(), [keep]);
        assert!(!s2.prune());
    }

    #[test]
    fn config_validation() {
        assert_eq!(ShelfConfig::from_table(None).unwrap().max, 8);
        for bad in ["max = 0", "max = 31", "maxx = 3"] {
            let t: toml::Table = toml::from_str(bad).unwrap();
            assert!(ShelfConfig::from_table(Some(&t)).is_err(), "{bad}");
        }
        assert_eq!(display_name(Path::new("C:/a/b/rapport.pdf")), "rapport.pdf");
    }
}
