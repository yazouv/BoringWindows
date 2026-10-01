//! Modification de config.toml par l'interface de réglages, sans perdre les
//! commentaires ni l'ordre du fichier.

use std::path::{Path, PathBuf};

use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, value};

use crate::{Config, ConfigError, DEFAULT_TOML};

/// Une valeur à écrire.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Bool(bool),
    Int(i64),
    Float(f64),
    StrList(Vec<String>),
    IntList(Vec<i64>),
}

pub struct ConfigEditor {
    path: PathBuf,
    doc: DocumentMut,
}

impl ConfigEditor {
    /// Ouvre le fichier (ou le modèle par défaut s'il n'existe pas encore).
    pub fn open(path: &Path) -> Result<Self, ConfigError> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => DEFAULT_TOML.to_owned(),
            Err(source) => {
                return Err(ConfigError::Io {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        Self::from_str(path, &text)
    }

    pub fn from_str(path: &Path, text: &str) -> Result<Self, ConfigError> {
        let doc = text
            .parse::<DocumentMut>()
            .map_err(|e| ConfigError::Invalid(vec![e.to_string()]))?;
        Ok(Self {
            path: path.to_owned(),
            doc,
        })
    }

    /// Écrit `v` à l'emplacement `key` (ex. `["theme", "accent"]`), en créant
    /// les sections manquantes.
    pub fn set(&mut self, key: &[&str], v: Value) {
        let Some((last, parents)) = key.split_last() else {
            return;
        };
        let table = self.table_mut(parents);
        let item = match v {
            Value::Str(s) => value(s),
            Value::Bool(b) => value(b),
            Value::Int(i) => value(i),
            Value::Float(f) => value(f),
            Value::StrList(list) => value(list.into_iter().collect::<Array>()),
            Value::IntList(list) => value(list.into_iter().collect::<Array>()),
        };
        // Garder la décoration (commentaire en fin de ligne) d'une valeur existante.
        match table.get_mut(last) {
            Some(Item::Value(old)) => {
                let decor = old.decor().clone();
                if let Item::Value(mut new) = item {
                    *new.decor_mut() = decor;
                    *old = new;
                }
            }
            _ => {
                table.insert(last, item);
            }
        }
    }

    /// Retire la clé `key` si elle existe (ex. une couleur qui masquait
    /// celle du thème).
    pub fn remove(&mut self, key: &[&str]) {
        let Some((last, parents)) = key.split_last() else {
            return;
        };
        let mut item = self.doc.as_item_mut();
        for part in parents {
            match item.get_mut(part) {
                Some(next) => item = next,
                None => return,
            }
        }
        if let Some(table) = item.as_table_like_mut() {
            table.remove(last);
        }
    }

    /// Config correspondant au texte en cours (thèmes cherchés à côté du fichier).
    pub fn config(&self) -> Result<Config, ConfigError> {
        Config::parse(
            &self.text(),
            self.path.parent().unwrap_or(std::path::Path::new(".")),
        )
    }

    /// Calendriers déclarés : (nom, lien).
    pub fn calendar_sources(&self) -> Vec<(String, String)> {
        self.sources()
            .map(|a| {
                a.iter()
                    .map(|t| {
                        let get =
                            |k: &str| t.get(k).and_then(Item::as_str).unwrap_or("").to_owned();
                        (get("name"), get("url"))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn add_calendar_source(&mut self, name: &str, url: &str) {
        self.add_calendar_account(name, url, None);
    }

    /// Ajoute un calendrier ; `caldav` = (identifiant, mot de passe).
    pub fn add_calendar_account(&mut self, name: &str, url: &str, caldav: Option<(&str, &str)>) {
        let calendar = self.table_mut(&["modules", "calendar"]);
        if !matches!(calendar.get("sources"), Some(Item::ArrayOfTables(_))) {
            calendar.insert("sources", Item::ArrayOfTables(ArrayOfTables::new()));
        }
        let mut entry = Table::new();
        if !name.trim().is_empty() {
            entry.insert("name", value(name.trim()));
        }
        if let Some((username, password)) = caldav {
            entry.insert("kind", value("caldav"));
            entry.insert("username", value(username.trim()));
            entry.insert("password", value(password));
        }
        entry.insert("url", value(url.trim()));
        if let Some(Item::ArrayOfTables(sources)) = calendar.get_mut("sources") {
            sources.push(entry);
        }
    }

    /// Valeurs du calendrier `index` (lien, mot de passe) qui sont des
    /// `secret:<id>` : à supprimer du coffre avec le calendrier.
    pub fn calendar_source_secrets(&self, index: usize) -> Vec<String> {
        let Some(sources) = self.sources() else {
            return Vec::new();
        };
        let Some(t) = sources.get(index) else {
            return Vec::new();
        };
        ["url", "password"]
            .iter()
            .filter_map(|k| t.get(k).and_then(Item::as_str))
            .filter(|v| v.trim().starts_with("secret:"))
            .map(str::to_owned)
            .collect()
    }

    pub fn remove_calendar_source(&mut self, index: usize) {
        let calendar = self.table_mut(&["modules", "calendar"]);
        if let Some(Item::ArrayOfTables(sources)) = calendar.get_mut("sources")
            && index < sources.len()
        {
            sources.remove(index);
            if sources.is_empty() {
                calendar.remove("sources");
            }
        }
    }

    /// Texte du fichier tel qu'il serait écrit.
    pub fn text(&self) -> String {
        self.doc.to_string()
    }

    /// Valide puis écrit le fichier (atomiquement). Rien n'est écrit si le
    /// résultat est invalide : le fichier reste toujours chargeable.
    pub fn save(&self) -> Result<Config, ConfigError> {
        let text = self.text();
        let config = self.config()?;
        let io_err = |source| ConfigError::Io {
            path: self.path.clone(),
            source,
        };
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(io_err)?;
        }
        let tmp = self.path.with_extension("toml.bw-tmp");
        std::fs::write(&tmp, &text).map_err(io_err)?;
        std::fs::rename(&tmp, &self.path).map_err(io_err)?;
        Ok(config)
    }

    fn sources(&self) -> Option<&ArrayOfTables> {
        self.doc
            .get("modules")?
            .get("calendar")?
            .get("sources")?
            .as_array_of_tables()
    }

    /// Table `[a.b.c]`, créée si besoin. Une valeur du même nom qui ne serait
    /// pas une table est remplacée.
    fn table_mut(&mut self, path: &[&str]) -> &mut Table {
        let mut table = self.doc.as_table_mut();
        for key in path {
            let entry = table.entry(key).or_insert_with(|| {
                let mut t = Table::new();
                t.set_implicit(true);
                Item::Table(t)
            });
            if !entry.is_table() {
                *entry = Item::Table(Table::new());
            }
            table = entry.as_table_mut().expect("table");
        }
        table
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpenOn;

    fn editor(text: &str) -> ConfigEditor {
        ConfigEditor::from_str(Path::new("config.toml"), text).unwrap()
    }

    #[test]
    fn remove_keys() {
        let mut e = ConfigEditor::from_str(
            Path::new("config.toml"),
            "[theme]\nname = \"light\"\naccent = \"#FF0000\"\n",
        )
        .unwrap();
        e.remove(&["theme", "accent"]);
        e.remove(&["theme", "absent"]);
        e.remove(&["absent", "x"]);
        assert_eq!(e.text(), "[theme]\nname = \"light\"\n");
    }

    #[test]
    fn keeps_comments_and_creates_sections() {
        let mut e = editor(DEFAULT_TOML);
        e.set(&["general", "open_on"], Value::Str("click".into()));
        e.set(&["theme", "accent"], Value::Str("#112233".into()));
        e.set(
            &["modules", "media", "ignore"],
            Value::StrList(vec!["msedge".into()]),
        );
        let text = e.text();
        assert!(text.contains("# Ouverture de l'île : \"hover\" (survol) ou \"click\"."));
        assert!(text.contains("open_on = \"click\""));
        assert!(text.contains("ignore = [\"msedge\"]"));
        let c = Config::from_toml_str(&text).unwrap();
        assert_eq!(c.general.open_on, OpenOn::Click);

        // Fichier vide : les sections sont créées.
        let mut e = editor("");
        e.set(&["modules", "claude", "sound"], Value::Bool(false));
        e.set(&["theme", "animation_ms"], Value::Int(0));
        let c = Config::from_toml_str(&e.text()).unwrap();
        assert_eq!(c.theme.animation_ms, 0);
        assert!(e.text().contains("[modules.claude]"), "{}", e.text());
    }

    #[test]
    fn caldav_account_and_secrets() {
        let mut e = editor(DEFAULT_TOML);
        e.add_calendar_account(
            "iCloud",
            "https://caldav.icloud.com/",
            Some(("moi@icloud.com", "secret:caldav-1")),
        );
        e.add_calendar_account("Lien", "secret:ics-2", None);
        e.add_calendar_source("Pub", "https://x/y.ics");
        assert_eq!(
            e.calendar_source_secrets(0),
            vec!["secret:caldav-1".to_owned()]
        );
        assert_eq!(e.calendar_source_secrets(1), vec!["secret:ics-2".to_owned()]);
        assert!(e.calendar_source_secrets(2).is_empty());
        assert!(e.calendar_source_secrets(9).is_empty());
        let text = e.text();
        assert!(text.contains("kind = \"caldav\""), "{text}");
        e.config().expect("config valide");
    }

    #[test]
    fn calendar_sources_round_trip() {
        let mut e = editor(DEFAULT_TOML);
        assert!(e.calendar_sources().is_empty());
        e.add_calendar_source("Pro", "https://a/b.ics");
        e.add_calendar_source("", "C:/x.ics");
        assert_eq!(
            e.calendar_sources(),
            vec![
                ("Pro".to_owned(), "https://a/b.ics".to_owned()),
                (String::new(), "C:/x.ics".to_owned())
            ]
        );
        let text = e.text();
        assert!(text.contains("[[modules.calendar.sources]]"), "{text}");
        assert!(Config::from_toml_str(&text).is_ok());

        e.remove_calendar_source(0);
        assert_eq!(e.calendar_sources().len(), 1);
        e.remove_calendar_source(0);
        e.remove_calendar_source(5);
        assert!(e.calendar_sources().is_empty());
        // Plus aucun bloc actif (l'exemple commenté du modèle reste).
        assert!(
            !e.text()
                .lines()
                .any(|l| l.starts_with("[[modules.calendar.sources]]"))
        );
    }

    #[test]
    fn save_refuses_invalid_and_writes_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "# mon commentaire\n[general]\nopen_on = \"hover\"\n").unwrap();

        let mut e = ConfigEditor::open(&path).unwrap();
        e.set(&["theme", "animation_ms"], Value::Int(99_999));
        assert!(e.save().is_err());
        assert!(std::fs::read_to_string(&path).unwrap().contains("hover"));

        e.set(&["theme", "animation_ms"], Value::Int(100));
        e.set(&["general", "open_on"], Value::Str("click".into()));
        let c = e.save().unwrap();
        assert_eq!(c.theme.animation_ms, 100);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# mon commentaire"));
        assert!(text.contains("open_on = \"click\""));
    }
}
