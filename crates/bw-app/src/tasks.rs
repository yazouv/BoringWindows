//! Tâches rapides : une liste de cases à cocher dans un fichier Markdown
//! (`tasks.md`, à côté de config.toml). Le fichier reste lisible et modifiable
//! à la main (Obsidian, VS Code…) : seules les lignes `- [ ] …` / `- [x] …`
//! sont des tâches, tout le reste (titres, notes) est conservé tel quel.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Section `[modules.tasks]` de config.toml.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TasksConfig {
    pub enabled: bool,
    /// Fichier des tâches ; vide : `tasks.md` à côté de config.toml.
    pub file: String,
}

impl TasksConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        Ok(match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        })
    }

    /// Le fichier des tâches, `config_dir` étant le dossier de config.toml.
    pub fn path(&self, config_dir: &Path) -> PathBuf {
        let file = self.file.trim();
        if file.is_empty() {
            config_dir.join("tasks.md")
        } else {
            config_dir.join(file)
        }
    }
}

/// Longueur maximale d'une tâche (caractères).
const MAX_LEN: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Task {
        /// Retrait et puce d'origine (`  - `), conservés à l'écriture.
        prefix: String,
        done: bool,
        text: String,
    },
    /// Tout le reste du fichier, rendu à l'identique.
    Other(String),
}

/// Une tâche telle que l'île l'affiche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Rang parmi les tâches du fichier (ordre du fichier) : sert aux actions.
    pub ordinal: usize,
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Tasks {
    lines: Vec<Line>,
}

impl Tasks {
    pub fn parse(text: &str) -> Self {
        Self {
            lines: text.lines().map(parse_line).collect(),
        }
    }

    /// Fichier absent ou illisible : liste vide.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .map(|t| Self::parse(&t))
            .unwrap_or_default()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            match line {
                Line::Task { prefix, done, text } => {
                    out.push_str(prefix);
                    out.push_str(if *done { "[x] " } else { "[ ] " });
                    out.push_str(text);
                }
                Line::Other(text) => out.push_str(text),
            }
            out.push('\n');
        }
        out
    }

    /// Écrit le fichier d'un coup (fichier temporaire puis renommage).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("md.tmp");
        std::fs::write(&tmp, self.render())?;
        std::fs::rename(&tmp, path)
    }

    fn tasks(&self) -> impl Iterator<Item = (usize, bool, &str)> {
        self.lines
            .iter()
            .filter_map(|l| match l {
                Line::Task { done, text, .. } => Some((*done, text.as_str())),
                Line::Other(_) => None,
            })
            .enumerate()
            .map(|(i, (done, text))| (i, done, text))
    }

    /// Les tâches à afficher : celles à faire d'abord, dans l'ordre du fichier,
    /// puis les faites.
    pub fn display(&self) -> Vec<Item> {
        let mut items: Vec<Item> = self
            .tasks()
            .map(|(ordinal, done, text)| Item {
                ordinal,
                text: text.to_owned(),
                done,
            })
            .collect();
        items.sort_by_key(|i| i.done);
        items
    }

    /// Tâches pas encore faites.
    pub fn open_count(&self) -> usize {
        self.tasks().filter(|(_, done, _)| !done).count()
    }

    pub fn done_count(&self) -> usize {
        self.tasks().filter(|(_, done, _)| *done).count()
    }

    /// Ajoute une tâche à la suite des autres. `false` si le texte est vide.
    pub fn add(&mut self, text: &str) -> bool {
        let text = clean(text);
        if text.is_empty() {
            return false;
        }
        let task = Line::Task {
            prefix: "- ".to_owned(),
            done: false,
            text,
        };
        match self
            .lines
            .iter()
            .rposition(|l| matches!(l, Line::Task { .. }))
        {
            Some(last) => self.lines.insert(last + 1, task),
            None => self.lines.push(task),
        }
        true
    }

    fn task_mut(&mut self, ordinal: usize) -> Option<&mut Line> {
        self.lines
            .iter_mut()
            .filter(|l| matches!(l, Line::Task { .. }))
            .nth(ordinal)
    }

    /// Coche ou décoche. `false` si la tâche n'existe plus.
    pub fn toggle(&mut self, ordinal: usize) -> bool {
        match self.task_mut(ordinal) {
            Some(Line::Task { done, .. }) => {
                *done = !*done;
                true
            }
            _ => false,
        }
    }

    pub fn remove(&mut self, ordinal: usize) -> bool {
        let Some(target) = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| matches!(l, Line::Task { .. }))
            .nth(ordinal)
            .map(|(at, _)| at)
        else {
            return false;
        };
        self.lines.remove(target);
        true
    }

    /// Retire les tâches faites. Retourne combien.
    pub fn clear_done(&mut self) -> usize {
        let before = self.lines.len();
        self.lines
            .retain(|l| !matches!(l, Line::Task { done: true, .. }));
        before - self.lines.len()
    }
}

/// Une ligne sur une seule ligne, sans espaces superflus, de longueur bornée.
fn clean(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    one_line.chars().take(MAX_LEN).collect()
}

/// `- [ ] texte`, `* [x] texte`, `  + [X] texte` : une tâche. Le reste, non.
fn parse_line(line: &str) -> Line {
    let other = || Line::Other(line.to_owned());
    let (indent, rest) = line.split_at(line.len() - line.trim_start().len());
    let mut chars = rest.chars();
    let Some(bullet @ ('-' | '*' | '+')) = chars.next() else {
        return other();
    };
    let after = chars.as_str();
    let spaces = after.len() - after.trim_start_matches(' ').len();
    if spaces == 0 {
        return other();
    }
    let body = &after[spaces..];
    let (done, text) = if let Some(text) = body.strip_prefix("[ ] ") {
        (false, text)
    } else if let Some(text) = body
        .strip_prefix("[x] ")
        .or_else(|| body.strip_prefix("[X] "))
    {
        (true, text)
    } else {
        return other();
    };
    let text = text.trim();
    if text.is_empty() {
        return other();
    }
    Line::Task {
        // Retrait, puce et espaces d'origine jusqu'à la case.
        prefix: format!("{indent}{bullet}{}", " ".repeat(spaces)),
        done,
        text: text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "# Courses\n\n- [ ] Acheter du lait\n- [x] Appeler Marie\nUne note libre.\n* [ ] Payer la facture\n";

    #[test]
    fn parses_tasks_and_keeps_everything_else() {
        let t = Tasks::parse(FILE);
        let items = t.display();
        assert_eq!(items.len(), 3);
        // Les tâches à faire d'abord, dans l'ordre du fichier ; les faites à la fin.
        assert_eq!(items[0].text, "Acheter du lait");
        assert_eq!(items[1].text, "Payer la facture");
        assert_eq!(items[2].text, "Appeler Marie");
        assert!(items[2].done);
        assert_eq!(t.open_count(), 2);
        assert_eq!(t.done_count(), 1);
        // Aucune perte à la réécriture.
        assert_eq!(t.render(), FILE);
    }

    #[test]
    fn non_tasks_are_left_alone() {
        for line in [
            "- pas une case",
            "-[ ] collé",
            "[ ] sans puce",
            "- [] vide",
            "- [ ]",
            "- [ ]    ",
            "- [y] autre lettre",
            "texte - [ ] au milieu",
            "",
        ] {
            assert!(matches!(parse_line(line), Line::Other(_)), "{line:?}");
        }
        for line in [
            "- [ ] ok",
            "* [x] ok",
            "+ [X] ok",
            "  - [ ] imbriquée",
            "-  [ ] deux espaces",
        ] {
            assert!(matches!(parse_line(line), Line::Task { .. }), "{line:?}");
        }
    }

    #[test]
    fn indentation_and_bullets_survive() {
        let text = "  - [ ] imbriquée\n* [x] étoile\n";
        assert_eq!(Tasks::parse(text).render(), text);
    }

    #[test]
    fn adds_after_the_last_task() {
        let mut t = Tasks::parse(FILE);
        assert!(t.add("  Nouvelle   tâche \n suite "));
        let rendered = t.render();
        assert!(rendered.ends_with("* [ ] Payer la facture\n- [ ] Nouvelle tâche suite\n"));
        assert_eq!(t.open_count(), 3);

        // Sans tâche : à la fin du fichier.
        let mut notes = Tasks::parse("# Notes\ntexte\n");
        assert!(notes.add("première"));
        assert_eq!(notes.render(), "# Notes\ntexte\n- [ ] première\n");
        // Une tâche vide n'est pas ajoutée.
        assert!(!notes.add("   \n  "));
        assert_eq!(notes.open_count(), 1);
        // Bornée en longueur.
        assert!(notes.add(&"x".repeat(500)));
        assert_eq!(
            notes.display().last().map(|i| i.text.chars().count()),
            Some(MAX_LEN)
        );
    }

    #[test]
    fn toggles_and_removes_by_file_order() {
        let mut t = Tasks::parse(FILE);
        // Rang 0 : « Acheter du lait », 1 : « Appeler Marie », 2 : « Payer la facture ».
        assert!(t.toggle(0));
        assert!(t.render().contains("- [x] Acheter du lait"));
        assert!(t.toggle(1));
        assert!(t.render().contains("- [ ] Appeler Marie"));
        assert!(t.remove(2));
        assert!(!t.render().contains("Payer"));
        // Une note libre n'est jamais touchée, et un rang inconnu est refusé.
        assert!(t.render().contains("Une note libre."));
        assert!(!t.toggle(9));
        assert!(!t.remove(9));
    }

    #[test]
    fn clears_done_tasks() {
        let mut t = Tasks::parse(FILE);
        assert_eq!(t.clear_done(), 1);
        assert_eq!(t.done_count(), 0);
        assert!(!t.render().contains("Marie"));
        assert_eq!(t.clear_done(), 0);
    }

    #[test]
    fn saves_and_reloads_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sous-dossier").join("tasks.md");
        // Fichier absent : liste vide.
        assert_eq!(Tasks::load(&path), Tasks::default());
        let mut t = Tasks::load(&path);
        t.add("écrire les tests");
        t.save(&path).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "- [ ] écrire les tests\n"
        );
        assert_eq!(Tasks::load(&path), t);
        // Pas de fichier temporaire oublié.
        assert!(!path.with_extension("md.tmp").exists());
    }

    #[test]
    fn config_paths() {
        let dir = Path::new("conf");
        assert_eq!(TasksConfig::default().path(dir), dir.join("tasks.md"));
        let custom: toml::Table =
            toml::from_str("file = \"notes/todo.md\"\nenabled = true").unwrap();
        let c = TasksConfig::from_table(Some(&custom)).unwrap();
        assert!(c.enabled);
        assert_eq!(c.path(dir), dir.join("notes/todo.md"));
        let bad: toml::Table = toml::from_str("couleur = 1").unwrap();
        assert!(TasksConfig::from_table(Some(&bad)).is_err());
    }
}
