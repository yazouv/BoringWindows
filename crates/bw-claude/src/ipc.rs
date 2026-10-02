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

/// `path` (socket ou dossier) doit nous appartenir et n'être modifiable que par
/// nous. Sinon, un autre compte de la machine aurait pu le créer avant l'app
/// pour recevoir les événements et répondre aux demandes de permission.
#[cfg(unix)]
pub fn check_private(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path)?;
    // SAFETY: geteuid n'a pas de précondition.
    let uid = unsafe { libc::geteuid() };
    if meta.uid() != uid || meta.mode() & 0o022 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "{} appartient à un autre compte ou est modifiable par d'autres",
                path.display()
            ),
        ));
    }
    Ok(())
}

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
                // Un dossier existant n'est pas recréé : il doit déjà être à nous.
                super::check_private(dir)?;
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

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn private_paths_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(check_private(dir.path()).is_ok());

        // Dossier modifiable par d'autres : un intrus aurait pu y placer la socket.
        let open = dir.path().join("ouvert");
        std::fs::create_dir(&open).unwrap();
        std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(check_private(&open).is_err());
        // L'app refuse d'y écouter.
        let socket = open.join("boringwindows.sock");
        let err = Listener::bind(socket.to_str().unwrap()).err().unwrap();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn socket_created_by_the_app_passes_the_check() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("app").join("boringwindows.sock");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let _listener = Listener::bind(socket.to_str().unwrap()).unwrap();
            assert!(check_private(socket.parent().unwrap()).is_ok());
            assert!(check_private(&socket).is_ok());
        });
    }
}
