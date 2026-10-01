//! Canal local entre le relais et l'app : named pipe sous Windows, socket Unix
//! ailleurs. Une connexion = un message JSON sur une ligne, et pour les
//! demandes de permission une ligne de réponse.

use serde::{Deserialize, Serialize};

use crate::event::HookEvent;

pub const PROTOCOL_VERSION: u32 = 1;
/// Taille maximale d'un message (le relais n'envoie que des résumés).
pub const MAX_LINE: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub v: u32,
    pub wants_reply: bool,
    /// Processus parents du relais (du plus proche au plus lointain), pour
    /// retrouver la fenêtre du terminal.
    #[serde(default)]
    pub ancestors: Vec<u32>,
    /// Fenêtre console héritée par le relais (Windows), si elle existe.
    #[serde(default)]
    pub console_window: Option<i64>,
    pub event: HookEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Deny,
    /// Laisser Claude Code demander dans le terminal.
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub decision: Decision,
}

/// Adresse du canal. `BW_IPC_ENDPOINT` la remplace (tests, instances parallèles).
pub fn endpoint() -> String {
    if let Ok(e) = std::env::var("BW_IPC_ENDPOINT") {
        return e;
    }
    default_endpoint()
}

#[cfg(windows)]
fn default_endpoint() -> String {
    let user: String = std::env::var("USERNAME")
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    format!(r"\\.\pipe\boringwindows-{user}")
}

#[cfg(unix)]
fn default_endpoint() -> String {
    // XDG_RUNTIME_DIR est privé (0700). Sinon, un dossier à nous dans /tmp.
    let dir = match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => std::env::temp_dir().join(format!(
            "boringwindows-{}",
            std::env::var("USER").unwrap_or_default()
        )),
    };
    dir.join("boringwindows.sock")
        .to_string_lossy()
        .into_owned()
}

pub use server::Listener;

#[cfg(windows)]
mod server {
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    /// Serveur de named pipe : une instance libre attend toujours un client.
    pub struct Listener {
        name: String,
        next: NamedPipeServer,
    }

    impl Listener {
        pub fn bind(name: &str) -> std::io::Result<Self> {
            // `first_pipe_instance` : échoue si un autre processus occupe déjà
            // le nom, au lieu de partager le pipe avec lui.
            let next = ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .create(name)?;
            Ok(Self {
                name: name.to_owned(),
                next,
            })
        }

        pub async fn accept(&mut self) -> std::io::Result<NamedPipeServer> {
            self.next.connect().await?;
            let fresh = ServerOptions::new()
                .reject_remote_clients(true)
                .create(&self.name)?;
            Ok(std::mem::replace(&mut self.next, fresh))
        }
    }
}

#[cfg(unix)]
mod server {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::path::{Path, PathBuf};

    use tokio::net::{UnixListener, UnixStream};

    pub struct Listener {
        inner: UnixListener,
        path: PathBuf,
    }

    impl Listener {
        pub fn bind(path: &str) -> std::io::Result<Self> {
            let path = PathBuf::from(path);
            if let Some(dir) = path.parent() {
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(dir)?;
            }
            // Socket orpheline d'un lancement précédent.
            let _ = std::fs::remove_file(&path);
            let inner = UnixListener::bind(&path)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            Ok(Self { inner, path })
        }

        pub async fn accept(&mut self) -> std::io::Result<UnixStream> {
            Ok(self.inner.accept().await?.0)
        }

        pub fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
