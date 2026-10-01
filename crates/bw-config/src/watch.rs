use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

use crate::{Config, ConfigError};

/// Garde la surveillance active tant qu'elle est en vie.
pub struct ConfigWatcher {
    _debouncer: Debouncer<RecommendedWatcher>,
}

/// Surveille `path` et appelle `on_change` (sur un thread de `notify`) à chaque
/// modification, avec la nouvelle config ou l'erreur de chargement.
///
/// C'est le dossier parent qui est surveillé : beaucoup d'éditeurs remplacent
/// le fichier au lieu de le réécrire, ce qui casserait une surveillance directe.
pub fn watch<F>(path: &Path, mut on_change: F) -> Result<ConfigWatcher, ConfigError>
where
    F: FnMut(Result<Config, ConfigError>) + Send + 'static,
{
    let dir = path.parent().unwrap_or(Path::new(".")).to_owned();
    let target: PathBuf = path.to_owned();
    let file_name = path.file_name().map(ToOwned::to_owned);

    let io_err = |e: notify_debouncer_mini::notify::Error| ConfigError::Io {
        path: dir.clone(),
        source: std::io::Error::other(e),
    };

    let mut debouncer = new_debouncer(
        Duration::from_millis(150),
        move |res: DebounceEventResult| match res {
            Ok(events) => {
                if events
                    .iter()
                    .any(|e| e.path.file_name() == file_name.as_deref())
                {
                    on_change(load(&target));
                }
            }
            Err(e) => log::warn!("surveillance de la config : {e}"),
        },
    )
    .map_err(io_err)?;

    debouncer
        .watcher()
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(io_err)?;

    Ok(ConfigWatcher {
        _debouncer: debouncer,
    })
}

fn load(path: &Path) -> Result<Config, ConfigError> {
    let s = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_owned(),
        source,
    })?;
    Config::from_toml_str(&s)
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
        let _watcher = watch(&path, move |res| tx.send(res).unwrap()).unwrap();
        let next = || {
            rx.recv_timeout(Duration::from_secs(10))
                .expect("rechargement attendu")
        };

        // Un autre fichier du dossier ne déclenche rien.
        std::fs::write(dir.path().join("other.txt"), "x").unwrap();
        std::fs::write(&path, "[general]\nopen_on = \"click\"").unwrap();
        assert_eq!(next().unwrap().general.open_on, OpenOn::Click);

        std::fs::write(&path, "[general]\nopen_on = \"never\"").unwrap();
        assert!(next().is_err());
    }
}
