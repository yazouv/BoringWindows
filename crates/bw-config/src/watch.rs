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
    // FSEvents (macOS) donne les chemins résolus (`/var` → `/private/var`) :
    // sans ça, `starts_with` ne reconnaîtrait jamais le dossier des thèmes.
    // Pas sous Windows, où `canonicalize` renvoie un chemin `\\?\…`.
    #[cfg(target_os = "macos")]
    let dir = dir.canonicalize().unwrap_or(dir);
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
        // L'écriture est refaite chaque seconde : FSEvents (macOS) démarre en
        // différé et perd les modifications faites juste après `watch`. Refaire
        // la même écriture ne notifie rien de plus (résultat inchangé).
        let wait_for = |write: &dyn Fn(), pred: &dyn Fn(&Result<Config, ConfigError>) -> bool| {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                write();
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                assert!(!left.is_zero(), "rechargement attendu");
                if let Ok(res) = rx.recv_timeout(left.min(Duration::from_secs(1)))
                    && pred(&res)
                {
                    return;
                }
            }
        };
        let write =
            |file: PathBuf, text: &'static str| move || std::fs::write(&file, text).unwrap();
        let theme = themes_dir(dir.path()).join("perso.toml");

        // Un autre fichier du dossier ne déclenche rien de faux.
        std::fs::write(dir.path().join("other.txt"), "x").unwrap();
        wait_for(
            &write(path.clone(), "[general]\nopen_on = \"click\""),
            &|r| r.as_ref().is_ok_and(|c| c.general.open_on == OpenOn::Click),
        );

        wait_for(
            &write(path.clone(), "[general]\nopen_on = \"never\""),
            &|r| r.is_err(),
        );

        // Un thème personnel modifié recharge aussi la config.
        std::fs::create_dir(themes_dir(dir.path())).unwrap();
        std::fs::write(&theme, "corner_radius = 5.0").unwrap();
        wait_for(&write(path.clone(), "[theme]\nname = \"perso\""), &|r| {
            r.as_ref().is_ok_and(|c| c.theme.corner_radius == 5.0)
        });
        wait_for(&write(theme.clone(), "corner_radius = 9.0"), &|r| {
            r.as_ref().is_ok_and(|c| c.theme.corner_radius == 9.0)
        });
    }
}
