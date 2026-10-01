//! Installation des hooks dans `~/.claude/settings.json`.
//!
//! On ne touche qu'à nos entrées (reconnues à `bw-hook` dans la commande) ;
//! le reste du fichier est conservé tel quel, dans le même ordre. Une
//! sauvegarde datée est faite avant chaque écriture.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use serde_json::{Map, Value, json};

/// Événements écoutés.
pub const HOOK_EVENTS: [&str; 7] = [
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "Notification",
    "Stop",
];
/// Événements filtrés par outil : `matcher` à `*`.
const TOOL_EVENTS: [&str; 2] = ["PreToolUse", "PermissionRequest"];
/// Marqueur de nos entrées.
const MARKER: &str = "bw-hook";
/// Timeouts (s) posés dans settings.json. Le relais se limite à 300 ms pour
/// les événements simples ; la permission attend la réponse dans l'île.
const TIMEOUT: u64 = 5;
const PERMISSION_TIMEOUT: u64 = 300;

/// Dossier de config de Claude Code (`CLAUDE_CONFIG_DIR` ou `~/.claude`).
pub fn claude_dir() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".claude")))
        .unwrap_or_else(|| PathBuf::from(".claude"))
}

/// Emplacement de la copie du relais (stable même si l'app est recompilée ou déplacée).
pub fn hook_binary_path() -> PathBuf {
    let name = if cfg!(windows) {
        "bw-hook.exe"
    } else {
        "bw-hook"
    };
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("BoringWindows")
        .join("bin")
        .join(name)
}

/// Commande de hook. Chemin avec des `/` : compris par bash comme par cmd.
pub fn hook_command(binary: &Path) -> String {
    format!("\"{}\" hook", binary.to_string_lossy().replace('\\', "/"))
}

fn is_ours(hook: &Value) -> bool {
    hook.get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains(MARKER))
}

pub fn is_installed(settings: &Value) -> bool {
    settings
        .get("hooks")
        .and_then(Value::as_object)
        .is_some_and(|hooks| {
            hooks.values().flat_map(entries).any(|entry| {
                entry
                    .get("hooks")
                    .and_then(Value::as_array)
                    .is_some_and(|hs| hs.iter().any(is_ours))
            })
        })
}

fn entries(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}

/// Retire nos hooks, en supprimant les groupes et événements devenus vides.
pub fn without_hooks(mut settings: Value) -> Value {
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return settings;
    };
    for groups in hooks.values_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        for group in groups.iter_mut() {
            if let Some(hs) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                hs.retain(|h| !is_ours(h));
            }
        }
        groups.retain(|g| {
            g.get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|hs| !hs.is_empty())
        });
    }
    hooks.retain(|_, groups| groups.as_array().is_none_or(|g| !g.is_empty()));
    if hooks.is_empty()
        && let Some(obj) = settings.as_object_mut()
    {
        obj.remove("hooks");
    }
    settings
}

/// Ajoute nos hooks (en remplaçant une éventuelle installation précédente).
pub fn with_hooks(settings: Value, command: &str) -> anyhow::Result<Value> {
    let mut settings = without_hooks(settings);
    let root = settings
        .as_object_mut()
        .context("settings.json doit contenir un objet JSON")?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .context("« hooks » doit être un objet")?;

    for event in HOOK_EVENTS {
        let timeout = if event == "PermissionRequest" {
            PERMISSION_TIMEOUT
        } else {
            TIMEOUT
        };
        let mut group = json!({
            "hooks": [{ "type": "command", "command": command, "timeout": timeout }]
        });
        if TOOL_EVENTS.contains(&event) {
            group["matcher"] = "*".into();
        }
        hooks
            .entry(event)
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .with_context(|| format!("« hooks.{event} » doit être une liste"))?
            .push(group);
    }
    Ok(settings)
}

pub struct Installer {
    pub settings_path: PathBuf,
    pub binary_path: PathBuf,
}

impl Default for Installer {
    fn default() -> Self {
        Self {
            settings_path: claude_dir().join("settings.json"),
            binary_path: hook_binary_path(),
        }
    }
}

#[derive(Debug)]
pub struct Report {
    pub settings_path: PathBuf,
    pub backup: Option<PathBuf>,
}

impl Installer {
    pub fn is_installed(&self) -> bool {
        self.read().is_ok_and(|s| is_installed(&s))
    }

    /// Copie `source` (l'exécutable courant) comme relais et déclare les hooks.
    pub fn install(&self, source: &Path) -> anyhow::Result<Report> {
        self.refresh_binary(source, true)?;
        let settings = self.read()?;
        let updated = with_hooks(settings, &hook_command(&self.binary_path))?;
        self.write(&updated)
    }

    pub fn uninstall(&self) -> anyhow::Result<Report> {
        let settings = self.read()?;
        let report = self.write(&without_hooks(settings))?;
        let _ = std::fs::remove_file(&self.binary_path);
        Ok(report)
    }

    /// Met à jour la copie du relais si l'exécutable a changé. Échoue sans
    /// gravité si le relais tourne à ce moment-là (fichier verrouillé).
    pub fn refresh_binary(&self, source: &Path, force: bool) -> anyhow::Result<bool> {
        let differs = || -> bool {
            let (Ok(a), Ok(b)) = (source.metadata(), self.binary_path.metadata()) else {
                return true;
            };
            a.len() != b.len() || a.modified().ok() > b.modified().ok()
        };
        if !force && !differs() {
            return Ok(false);
        }
        if let Some(dir) = self.binary_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::copy(source, &self.binary_path)
            .with_context(|| format!("copie du relais vers {}", self.binary_path.display()))?;
        Ok(true)
    }

    fn read(&self) -> anyhow::Result<Value> {
        match std::fs::read_to_string(&self.settings_path) {
            Ok(s) if s.trim().is_empty() => Ok(json!({})),
            Ok(s) => serde_json::from_str(&s).with_context(|| {
                format!("{} n'est pas du JSON valide", self.settings_path.display())
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
            Err(e) => {
                Err(e).with_context(|| format!("lecture de {}", self.settings_path.display()))
            }
        }
    }

    fn write(&self, settings: &Value) -> anyhow::Result<Report> {
        let path = &self.settings_path;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let backup = if path.exists() {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let backup = path.with_extension(format!("json.bak-{stamp}"));
            std::fs::copy(path, &backup)
                .with_context(|| format!("sauvegarde de {}", path.display()))?;
            Some(backup)
        } else {
            None
        };

        // Écriture atomique : fichier temporaire puis renommage.
        let tmp = path.with_extension("json.bw-tmp");
        let mut text = serde_json::to_string_pretty(settings)?;
        text.push('\n');
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path).with_context(|| format!("écriture de {}", path.display()))?;
        Ok(Report {
            settings_path: path.clone(),
            backup,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: &str = "\"C:/Users/me/AppData/Local/BoringWindows/bin/bw-hook.exe\" hook";

    fn user_settings() -> Value {
        json!({
            "model": "opus",
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "command", "command": "notify-send done" }] }],
                "PostToolUse": [{ "matcher": "Write", "hooks": [{ "type": "command", "command": "fmt.sh" }] }]
            },
            "permissions": { "allow": ["Bash(ls)"] }
        })
    }

    #[test]
    fn install_adds_every_event_and_keeps_user_hooks() {
        let s = with_hooks(user_settings(), CMD).unwrap();
        assert!(is_installed(&s));
        for event in HOOK_EVENTS {
            let groups = s["hooks"][event].as_array().unwrap();
            let ours = groups.iter().find(|g| is_ours(&g["hooks"][0])).unwrap();
            assert_eq!(ours["hooks"][0]["command"], CMD);
            assert_eq!(ours.get("matcher").is_some(), TOOL_EVENTS.contains(&event));
        }
        assert_eq!(
            s["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"],
            300
        );
        // Hooks de l'utilisateur intacts, ordre des clés conservé.
        assert_eq!(
            s["hooks"]["Stop"][0]["hooks"][0]["command"],
            "notify-send done"
        );
        assert_eq!(
            s["hooks"]["PostToolUse"],
            user_settings()["hooks"]["PostToolUse"]
        );
        let keys: Vec<_> = s.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["model", "hooks", "permissions"]);
    }

    #[test]
    fn install_is_idempotent_and_uninstall_restores() {
        let once = with_hooks(user_settings(), CMD).unwrap();
        let twice = with_hooks(once.clone(), CMD).unwrap();
        assert_eq!(once, twice);
        assert_eq!(without_hooks(twice), user_settings());
    }

    #[test]
    fn uninstall_removes_empty_hooks_section() {
        let s = with_hooks(json!({ "model": "opus" }), CMD).unwrap();
        assert_eq!(without_hooks(s), json!({ "model": "opus" }));
        assert!(!is_installed(&json!({})));
    }

    #[test]
    fn rejects_unexpected_shapes() {
        assert!(with_hooks(json!([1]), CMD).is_err());
        assert!(with_hooks(json!({ "hooks": { "Stop": {} } }), CMD).is_err());
    }

    #[test]
    fn command_uses_forward_slashes() {
        let cmd = hook_command(Path::new(r"C:\Users\me\bw-hook.exe"));
        assert_eq!(cmd, "\"C:/Users/me/bw-hook.exe\" hook");
    }

    #[test]
    fn installer_on_disk_with_backup() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("app");
        std::fs::write(&source, "binary").unwrap();
        let installer = Installer {
            settings_path: dir.path().join(".claude").join("settings.json"),
            binary_path: dir.path().join("bin").join("bw-hook"),
        };

        // Fichier absent : créé, sans sauvegarde.
        let report = installer.install(&source).unwrap();
        assert!(report.backup.is_none());
        assert!(installer.is_installed());
        assert_eq!(
            std::fs::read_to_string(&installer.binary_path).unwrap(),
            "binary"
        );
        assert!(!installer.refresh_binary(&source, false).unwrap());

        // Désinstallation : sauvegarde de la version précédente.
        let report = installer.uninstall().unwrap();
        let backup = report.backup.unwrap();
        assert!(is_installed(
            &serde_json::from_str(&std::fs::read_to_string(backup).unwrap()).unwrap()
        ));
        assert!(!installer.is_installed());
        assert!(!installer.binary_path.exists());

        // JSON invalide : on refuse d'y toucher.
        std::fs::write(&installer.settings_path, "{ oops").unwrap();
        assert!(installer.install(&source).is_err());
        assert_eq!(
            std::fs::read_to_string(&installer.settings_path).unwrap(),
            "{ oops"
        );
    }
}
