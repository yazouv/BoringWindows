//! Un plugin = un dossier `plugins/<nom>/` avec `plugin.toml` et un `.wasm`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

/// Contenu de `plugin.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Nom affiché ; par défaut le nom du dossier.
    #[serde(default)]
    pub name: String,
    /// Fichier WASM, dans le dossier du plugin.
    #[serde(default = "default_wasm")]
    pub wasm: String,
    /// Intervalle entre deux appels de `bw_update` (5 à 3600 secondes).
    #[serde(default = "default_interval")]
    pub interval_secs: u32,
}

fn default_wasm() -> String {
    "plugin.wasm".into()
}

fn default_interval() -> u32 {
    30
}

/// Un plugin trouvé sur le disque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSpec {
    /// Nom du dossier (identifiant).
    pub id: String,
    pub name: String,
    pub wasm_path: PathBuf,
    pub interval: Duration,
}

/// Dossier des plugins, à côté de config.toml.
pub fn plugins_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("plugins")
}

pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Plugins de `dir` : un résultat par sous-dossier contenant un `plugin.toml`
/// (les erreurs sont des messages lisibles, plugin par plugin).
pub fn discover(dir: &Path) -> Vec<Result<PluginSpec, String>> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut folders: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("plugin.toml").is_file())
        .collect();
    folders.sort();
    folders.iter().map(|f| read_spec(f)).collect()
}

fn read_spec(folder: &Path) -> Result<PluginSpec, String> {
    let id = folder
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let fail = |msg: String| format!("plugin {id} : {msg}");
    if !valid_name(&id) {
        return Err(fail(
            "nom de dossier invalide (lettres, chiffres, - et _)".into(),
        ));
    }
    let text =
        std::fs::read_to_string(folder.join("plugin.toml")).map_err(|e| fail(e.to_string()))?;
    let manifest: Manifest = toml::from_str(&text).map_err(|e| fail(e.message().to_owned()))?;
    if !(5..=3600).contains(&manifest.interval_secs) {
        return Err(fail("interval_secs doit être entre 5 et 3600".into()));
    }
    // Le .wasm reste dans le dossier du plugin : pas de chemin qui en sorte.
    if manifest.wasm.contains(['/', '\\'])
        || manifest.wasm.contains("..")
        || manifest.wasm.is_empty()
    {
        return Err(fail(
            "`wasm` doit être un nom de fichier du dossier du plugin".into(),
        ));
    }
    let wasm_path = folder.join(&manifest.wasm);
    if !wasm_path.is_file() {
        return Err(fail(format!("{} introuvable", manifest.wasm)));
    }
    Ok(PluginSpec {
        name: if manifest.name.trim().is_empty() {
            id.clone()
        } else {
            manifest.name.trim().to_owned()
        },
        id,
        wasm_path,
        interval: Duration::from_secs(manifest.interval_secs.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bw-plugins-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn plugin(root: &Path, id: &str, toml: &str, wasm: bool) {
        let p = root.join(id);
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("plugin.toml"), toml).unwrap();
        if wasm {
            std::fs::write(p.join("plugin.wasm"), [0, 97, 115, 109]).unwrap();
        }
    }

    #[test]
    fn discovers_valid_and_reports_invalid() {
        let root = dir("discover");
        plugin(
            &root,
            "ok",
            "name = \"Mon plugin\"\ninterval_secs = 10",
            true,
        );
        plugin(&root, "defaults", "", true);
        plugin(&root, "no-wasm", "", false);
        plugin(&root, "slow", "interval_secs = 1", true);
        plugin(&root, "escape", "wasm = \"../x.wasm\"", true);
        plugin(&root, "typo", "interval = 3", true);
        std::fs::create_dir_all(root.join("sans-manifeste")).unwrap();

        let found = discover(&root);
        let by_id = |id: &str| {
            found
                .iter()
                .find(|r| match r {
                    Ok(s) => s.id == id,
                    Err(e) => e.starts_with(&format!("plugin {id} ")),
                })
                .unwrap()
        };
        let ok = by_id("ok").as_ref().unwrap();
        assert_eq!(
            (ok.name.as_str(), ok.interval),
            ("Mon plugin", Duration::from_secs(10))
        );
        let d = by_id("defaults").as_ref().unwrap();
        assert_eq!(
            (d.name.as_str(), d.interval),
            ("defaults", Duration::from_secs(30))
        );
        for bad in ["no-wasm", "slow", "escape", "typo"] {
            assert!(by_id(bad).is_err(), "{bad}");
        }
        assert_eq!(found.len(), 6);
        assert!(discover(&root.join("absent")).is_empty());
    }
}
