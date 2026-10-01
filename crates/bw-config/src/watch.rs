use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

use crate::{Config, ConfigError, layouts_dir, themes_dir};

/// Garde la surveillance active tant qu'elle est en vie.
pub struct ConfigWatcher {
    _debouncer: Debouncer<RecommendedWatcher>,
}

/// Surveille `path` (et les thèmes personnels à côté) et appelle `on_change`
/// (sur un thread de `notify`) à chaque modification, avec la nouvelle config
/// ou l'erreur de chargement.
///
/// C'est le dossier parent qui est surveillé : beaucoup d'éditeurs remplacent
/// le fichier au lieu de le réécrire, ce qui casserait une surveillance directe.
pub fn watch<F>(path: &Path, mut on_change: F) -> Result<ConfigWatcher, ConfigError>
where
    F: FnMut(Result<Config, ConfigError>) + Send + 'static,
{
    let dir = path.parent().unwrap_or(Path::new(".")).to_owned();
    let themes = themes_dir(&dir);
    let layouts = layouts_dir(&dir);
    let target: PathBuf = path.to_owned();
    // Certains systèmes (inotify) signalent aussi les lectures : on ne prévient
    // que si le résultat change, sinon chaque rechargement en déclencherait un autre.
    let mut last = summary(&load(path));

    let io_err = |e: notify_debouncer_mini::notify::Error| ConfigError::Io {
        path: dir.clone(),
        source: std::io::Error::other(e),
    };

    let mut debouncer = new_debouncer(
        Duration::from_millis(150),
        move |res: DebounceEventResult| match res {
            Ok(events) => {
                let relevant = events.iter().any(|e| {
                    e.path.file_name() == target.file_name()
                        || (e.path.starts_with(&themes)
                            && e.path.extension().is_some_and(|x| x == "toml"))
                        || (e.path.starts_with(&layouts)
                            && e.path.extension().is_some_and(|x| x == "slint"))
                });
                if !relevant {
                    return;
                }
                let result = load(&target);
                let now = summary(&result);
                if now == last {
                    return;
                }
                last = now;
                on_change(result);
            }
            Err(e) => log::warn!("surveillance de la config : {e}"),
        },
    )
    .map_err(io_err)?;

    debouncer
        .watcher()
        .watch(&dir, RecursiveMode::Recursive)
        .map_err(io_err)?;

    Ok(ConfigWatcher {
        _debouncer: debouncer,
    })
}

/// Ce qui compte pour décider s'il faut prévenir.
fn summary(result: &Result<Config, ConfigError>) -> Result<Config, String> {
    match result {
        Ok(config) => Ok(config.clone()),
        Err(e) => Err(e.to_string()),
    }
}

fn load(path: &Path) -> Result<Config, ConfigError> {
    let s = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_owned(),
        source,
    })?;
    Config::parse(&s, path.parent().unwrap_or(Path::new(".")))
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use crate::OpenOn;

    #[test]
    fn reports_valid_and_invalid_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "").unwrap();

        let (tx, rx) = mpsc::channel();
        let _watcher = watch(&path, move |res| {
            let _ = tx.send(res);
        })
        .unwrap();
        // Une même écriture peut produire plusieurs notifications (création,
        // modification) : on attend le résultat voulu au lieu d'en compter.
        let wait_for = |pred: &dyn Fn(&Result<Config, ConfigError>) -> bool| {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                let res = rx.recv_timeout(left).expect("rechargement attendu");
                if pred(&res) {
                    return;
                }
            }
        };

        // Un autre fichier du dossier ne déclenche rien de faux.
        std::fs::write(dir.path().join("other.txt"), "x").unwrap();
        std::fs::write(&path, "[general]\nopen_on = \"click\"").unwrap();
        wait_for(&|r| r.as_ref().is_ok_and(|c| c.general.open_on == OpenOn::Click));

        std::fs::write(&path, "[general]\nopen_on = \"never\"").unwrap();
        wait_for(&|r| r.is_err());

        // Un thème personnel modifié recharge aussi la config.
        std::fs::create_dir(themes_dir(dir.path())).unwrap();
        std::fs::write(
            themes_dir(dir.path()).join("perso.toml"),
            "corner_radius = 5.0",
        )
        .unwrap();
        std::fs::write(&path, "[theme]\nname = \"perso\"").unwrap();
        wait_for(&|r| r.as_ref().is_ok_and(|c| c.theme.corner_radius == 5.0));
        std::fs::write(
            themes_dir(dir.path()).join("perso.toml"),
            "corner_radius = 9.0",
        )
        .unwrap();
        wait_for(&|r| r.as_ref().is_ok_and(|c| c.theme.corner_radius == 9.0));
    }
}
