//! Mises à jour depuis les releases GitHub : on cherche une version plus
//! récente, on télécharge le binaire de la plateforme, on vérifie son
//! empreinte SHA-256 et on remplace l'exécutable. La version qui tourne
//! continue jusqu'au prochain lancement.
//!
//! Tout est bloquant : à appeler hors du thread UI.

use std::path::{Path, PathBuf};

use anyhow::Context;
use bw_i18n::tr;
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Dépôt dont on suit les releases.
pub const REPO: &str = "yazouv/BoringWindows";

/// Variable d'environnement facultative : jeton GitHub (lecture seule) pour
/// suivre les releases d'un dépôt privé.
pub const TOKEN_VAR: &str = "BORINGWINDOWS_GITHUB_TOKEN";

/// Nom du binaire de cette plateforme dans les releases.
pub fn asset_name() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some("boringwindows-windows-x64.exe")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("boringwindows-macos-arm64.tar.gz")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("boringwindows-linux-x64.tar.gz")
    } else {
        None
    }
}

/// Version `x.y.z` (suffixe éventuel ignoré : `1.2.3-beta` → 1.2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('v');
        let core = s.split(['-', '+']).next()?;
        let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
        let v = Version(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(v)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Release plus récente que la version courante, avec un binaire pour nous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub version: Version,
    /// Page de la release (notes de version).
    pub page: String,
    asset_url: String,
    checksum_url: Option<String>,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    /// URL d'API : marche aussi pour un dépôt privé (avec jeton).
    url: String,
}

/// Exécutable lancé depuis un dossier `target` de cargo : on ne le remplace
/// pas (développement).
pub fn is_dev_build(exe: &Path) -> bool {
    exe.ancestors().any(|dir| {
        dir.file_name() == Some("target".as_ref()) && dir.with_file_name("Cargo.toml").is_file()
    })
}

/// Cherche une version plus récente que `current`. `Ok(None)` : à jour.
pub fn check(current: &str) -> anyhow::Result<Option<Update>> {
    check_repo(REPO, current)
}

fn check_repo(repo: &str, current: &str) -> anyhow::Result<Option<Update>> {
    check_url(
        &format!("https://api.github.com/repos/{repo}/releases/latest"),
        current,
    )
}

fn check_url(url: &str, current: &str) -> anyhow::Result<Option<Update>> {
    let current = Version::parse(current).context("version courante illisible")?;
    let token = token();
    let headers = headers("application/vnd.github+json", token.as_deref());
    let body = bw_net::get_text(url, &refs(&headers)).map_err(|e| {
        if token.is_none() && e.to_string().contains("404") {
            anyhow::anyhow!(tr!(
                "no release found (private repository? set {TOKEN_VAR})",
                "aucune release trouvée (dépôt privé ? définis {TOKEN_VAR})"
            ))
        } else {
            e
        }
    })?;
    let release: ApiRelease = serde_json::from_str(&body).context("réponse GitHub illisible")?;
    Ok(pick(release, current))
}

fn pick(release: ApiRelease, current: Version) -> Option<Update> {
    if release.draft || release.prerelease {
        return None;
    }
    let version = Version::parse(&release.tag_name)?;
    if version <= current {
        return None;
    }
    let name = asset_name()?;
    let asset = release.assets.iter().find(|a| a.name == name)?;
    let checksum = release
        .assets
        .iter()
        .find(|a| a.name == format!("{name}.sha256"));
    Some(Update {
        version,
        page: release.html_url,
        asset_url: asset.url.clone(),
        checksum_url: checksum.map(|a| a.url.clone()),
    })
}

/// Télécharge, vérifie et installe la mise à jour à la place de `exe`.
pub fn install(update: &Update, exe: &Path) -> anyhow::Result<()> {
    let token = token();
    let octets = headers("application/octet-stream", token.as_deref());
    let data = bw_net::get(&update.asset_url, &refs(&octets))
        .with_context(|| tr!("download failed", "téléchargement impossible"))?;
    let checksum_url = update.checksum_url.as_ref().with_context(|| {
        tr!(
            "the release has no checksum (.sha256)",
            "la release n'a pas d'empreinte (.sha256)"
        )
    })?;
    let expected = bw_net::get_text(checksum_url, &refs(&octets))?;
    verify(&data, &expected)?;

    let binary = if asset_name().is_some_and(|n| n.ends_with(".tar.gz")) {
        extract(&data, exe)?
    } else {
        data
    };
    replace(exe, &binary)
}

/// Supprime l'ancien exécutable laissé par une mise à jour (Windows).
pub fn cleanup(exe: &Path) {
    let _ = std::fs::remove_file(old_path(exe));
}

fn token() -> Option<String> {
    std::env::var(TOKEN_VAR)
        .ok()
        .filter(|t| !t.trim().is_empty())
}

fn headers<'a>(accept: &'a str, token: Option<&'a str>) -> Vec<(&'a str, String)> {
    let mut h = vec![
        ("Accept", accept.to_owned()),
        (
            "User-Agent",
            format!("BoringWindows/{}", env!("CARGO_PKG_VERSION")),
        ),
        ("X-GitHub-Api-Version", "2022-11-28".to_owned()),
    ];
    if let Some(t) = token {
        h.push(("Authorization", format!("Bearer {}", t.trim())));
    }
    h
}

fn refs<'a>(headers: &'a [(&'a str, String)]) -> Vec<(&'a str, &'a str)> {
    headers.iter().map(|(k, v)| (*k, v.as_str())).collect()
}

fn verify(data: &[u8], expected: &str) -> anyhow::Result<()> {
    // Format de sha256sum : « <hex>  <nom> ».
    let expected = expected.split_whitespace().next().unwrap_or_default();
    let actual: String = Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    anyhow::ensure!(
        actual.eq_ignore_ascii_case(expected),
        tr!(
            "checksum mismatch: corrupted download",
            "empreinte différente : téléchargement corrompu"
        )
    );
    Ok(())
}

/// Sort `boringwindows` d'une archive `.tar.gz` (avec `tar`, présent sur
/// macOS et Linux).
fn extract(archive: &[u8], exe: &Path) -> anyhow::Result<Vec<u8>> {
    let dir = exe
        .parent()
        .context("dossier de l'exécutable inconnu")?
        .join(".bw-update");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let tar = dir.join("update.tar.gz");
    std::fs::write(&tar, archive)?;
    let status = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(&tar)
        .arg("-C")
        .arg(&dir)
        .status()?;
    anyhow::ensure!(status.success(), "tar : {status}");
    let binary = std::fs::read(dir.join("boringwindows"))?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(binary)
}

fn old_path(exe: &Path) -> PathBuf {
    let mut name = exe.file_name().unwrap_or_default().to_owned();
    name.push(".old");
    exe.with_file_name(name)
}

/// Remplace `exe` par `binary`. Un exécutable en cours d'utilisation ne peut
/// pas être écrasé sous Windows, mais il peut être renommé : l'ancien devient
/// `<nom>.old`, supprimé au lancement suivant.
fn replace(exe: &Path, binary: &[u8]) -> anyhow::Result<()> {
    let mut name = exe.file_name().unwrap_or_default().to_owned();
    name.push(".new");
    let new = exe.with_file_name(name);
    std::fs::write(&new, binary)
        .with_context(|| tr!("cannot write {}", "impossible d'écrire {}", new.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755))?;
    }
    if cfg!(windows) {
        let old = old_path(exe);
        let _ = std::fs::remove_file(&old);
        std::fs::rename(exe, &old)?;
        if let Err(e) = std::fs::rename(&new, exe) {
            // On remet l'ancien en place.
            let _ = std::fs::rename(&old, exe);
            return Err(e.into());
        }
    } else {
        std::fs::rename(&new, exe)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(Version::parse("v0.2.10"), Some(Version(0, 2, 10)));
        assert_eq!(Version::parse("1.0.0-beta.1"), Some(Version(1, 0, 0)));
        assert_eq!(Version::parse("1.0"), None);
        assert_eq!(Version::parse("1.0.0.0"), None);
        assert!(Version(0, 10, 0) > Version(0, 9, 9));
        assert_eq!(Version(1, 2, 3).to_string(), "1.2.3");
    }

    fn release(tag: &str, assets: &[&str]) -> ApiRelease {
        ApiRelease {
            tag_name: tag.into(),
            html_url: format!("https://github.com/{REPO}/releases/tag/{tag}"),
            draft: false,
            prerelease: false,
            assets: assets
                .iter()
                .map(|n| ApiAsset {
                    name: (*n).into(),
                    url: format!("https://api.github.com/assets/{n}"),
                })
                .collect(),
        }
    }

    #[test]
    fn picks_only_newer_releases_with_our_binary() {
        let Some(name) = asset_name() else { return };
        let checksum = format!("{name}.sha256");
        let current = Version(0, 2, 0);

        let u = pick(release("v0.3.0", &[name, &checksum, "autre.zip"]), current).unwrap();
        assert_eq!(u.version, Version(0, 3, 0));
        assert!(u.asset_url.ends_with(name));
        assert!(u.checksum_url.unwrap().ends_with(".sha256"));

        assert_eq!(pick(release("v0.2.0", &[name]), current), None);
        assert_eq!(pick(release("v0.1.9", &[name]), current), None);
        assert_eq!(pick(release("v0.3.0", &["autre.zip"]), current), None);
        let mut pre = release("v0.3.0", &[name]);
        pre.prerelease = true;
        assert_eq!(pick(pre, current), None);
    }

    #[test]
    fn checksums() {
        // sha256("abc")
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(verify(b"abc", &format!("{abc}  boringwindows-linux-x64.tar.gz\n")).is_ok());
        assert!(verify(b"abd", abc).is_err());
        assert!(verify(b"abc", "").is_err());
    }

    #[test]
    fn replaces_the_executable() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("boringwindows.exe");
        std::fs::write(&exe, "v1").unwrap();
        replace(&exe, b"v2").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"v2");
        cleanup(&exe);
        assert!(!old_path(&exe).exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn extracts_tar_gz() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("boringwindows"), "binaire").unwrap();
        let tar = dir.path().join("a.tar.gz");
        let ok = std::process::Command::new("tar")
            .arg("-czf")
            .arg(&tar)
            .arg("-C")
            .arg(&src)
            .arg("boringwindows")
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            return; // pas de tar sur cette machine
        }
        let exe = dir.path().join("app").join("boringwindows");
        std::fs::create_dir(exe.parent().unwrap()).unwrap();
        let out = extract(&std::fs::read(&tar).unwrap(), &exe).unwrap();
        assert_eq!(out, b"binaire");
        assert!(!exe.parent().unwrap().join(".bw-update").exists());
    }

    /// Appel réel à l'API GitHub (réseau) : `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_public_repo() {
        let latest = check_repo("slint-ui/slint", "0.0.1").unwrap();
        // Slint publie des binaires, mais pas sous nos noms : rien à installer.
        assert_eq!(latest, None);
        assert!(check_repo("slint-ui/slint", "999.0.0").unwrap().is_none());
    }

    /// Serveur HTTP minimal : `routes(adresse)` donne les réponses (200) par
    /// chemin, le reste est en 404. Renvoie l'adresse (`http://127.0.0.1:port`).
    fn serve(routes: impl FnOnce(&str) -> Vec<(String, Vec<u8>)>) -> String {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes = routes(&base);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
                // En-têtes ignorés.
                while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                    line.clear();
                }
                let mut stream = &stream;
                match routes.iter().find(|(p, _)| *p == path) {
                    Some((_, body)) => {
                        let _ = write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(body);
                    }
                    None => {
                        let _ = stream
                            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    }
                }
            }
        });
        base
    }

    /// Parcours complet contre un faux GitHub local.
    #[test]
    fn checks_downloads_and_installs() {
        let Some(name) = asset_name() else { return };
        let dir = tempfile::tempdir().unwrap();
        let new_binary = b"nouvelle version".to_vec();
        let asset = if name.ends_with(".tar.gz") {
            let src = dir.path().join("src");
            std::fs::create_dir(&src).unwrap();
            std::fs::write(src.join("boringwindows"), &new_binary).unwrap();
            let tar = dir.path().join("a.tar.gz");
            let ok = std::process::Command::new("tar")
                .arg("-czf")
                .arg(&tar)
                .arg("-C")
                .arg(&src)
                .arg("boringwindows")
                .status()
                .is_ok_and(|s| s.success());
            if !ok {
                return;
            }
            std::fs::read(&tar).unwrap()
        } else {
            new_binary.clone()
        };
        let sum: String = Sha256::digest(&asset)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();

        let server = serve(|base| {
            let json = format!(
                r#"{{"tag_name":"v9.9.9","html_url":"{base}/page","assets":[
                    {{"name":"{name}","url":"{base}/asset"}},
                    {{"name":"{name}.sha256","url":"{base}/sum"}}]}}"#
            );
            vec![
                ("/latest".to_owned(), json.into_bytes()),
                ("/asset".to_owned(), asset.clone()),
                ("/sum".to_owned(), format!("{sum}  {name}\n").into_bytes()),
            ]
        });

        let update = check_url(&format!("{server}/latest"), "1.0.0")
            .unwrap()
            .expect("mise à jour attendue");
        assert_eq!(update.version, Version(9, 9, 9));
        assert!(
            check_url(&format!("{server}/latest"), "9.9.9")
                .unwrap()
                .is_none()
        );

        let exe = dir.path().join("app").join("boringwindows.exe");
        std::fs::create_dir(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, "ancienne version").unwrap();
        install(&update, &exe).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), new_binary);

        // Empreinte fausse : rien n'est remplacé.
        let bad = Update {
            checksum_url: Some(format!("{server}/latest")),
            ..update
        };
        assert!(install(&bad, &exe).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), new_binary);
    }

    #[test]
    fn dev_builds_are_detected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let exe = dir
            .path()
            .join("target")
            .join("debug")
            .join("boringwindows");
        assert!(is_dev_build(&exe));
        assert!(!is_dev_build(Path::new("/opt/boringwindows/boringwindows")));
    }
}
