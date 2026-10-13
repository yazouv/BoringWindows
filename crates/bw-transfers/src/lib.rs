//! Transferts de fichiers : la progression des copies de l'Explorateur et des
//! téléchargements des navigateurs, avec une barre dans la pilule repliée.
//!
//! - **Copies, déplacements, suppressions** (Windows) : l'Explorateur ouvre une
//!   fenêtre `OperationStatusWindow` dont le titre porte le pourcentage
//!   (« 17% terminé »). Un hook sur les changements de titre la suit sans
//!   rien interroger (`win_copy.rs`).
//! - **Téléchargements** : les navigateurs écrivent un fichier partiel
//!   (`.crdownload`, `.part`…) qui grossit, puis le renomment. On surveille le
//!   dossier de téléchargement et, tant qu'un partiel existe, on relève sa
//!   taille chaque seconde. Les navigateurs ne donnent pas la taille finale :
//!   la barre est alors indéterminée, avec la taille reçue et la vitesse.
//!
//! La logique (`parse_percent`, `partial_target`, `DownloadTracker`…) est pure.

mod config;
mod module;
#[cfg(windows)]
mod win_copy;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bw_i18n::{Lang, tr};

pub use config::TransfersConfig;
pub use module::{MODULE_ID, TransfersModule};

/// Progression publiée pour la barre de la pilule repliée.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferProgress {
    /// Un transfert est en cours (la barre est affichée).
    pub active: bool,
    /// Avancement (0 à 1) ; `None` si la taille finale est inconnue.
    pub fraction: Option<f32>,
}

impl TransferProgress {
    pub const IDLE: Self = Self {
        active: false,
        fraction: None,
    };
}

// --- Copies --------------------------------------------------------------------

/// Ce que le suivi des fenêtres de copie signale au module.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CopyEvent {
    /// La fenêtre existe ; `percent` est `None` tant qu'Explorer calcule.
    Update {
        window: isize,
        percent: Option<u8>,
        label: Option<String>,
    },
    /// La fenêtre s'est fermée (copie terminée ou annulée).
    Gone { window: isize },
}

/// Pourcentage porté par le titre de la fenêtre de copie (« 17% terminé »,
/// « 17 % complete », « En pause - 17% terminé »).
pub fn parse_percent(title: &str) -> Option<u8> {
    let at = title.find('%')?;
    let digits: String = title[..at]
        .trim_end()
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse::<u8>().ok().filter(|p| *p <= 100)
}

/// « Copie d'un élément de A vers B » : l'opération en clair, extraite du nom du
/// groupe de la fenêtre (« Copie d'un élément de A vers B - 17% terminé »).
pub fn operation_label(group_name: &str) -> Option<String> {
    let label = group_name.split(" - ").next()?.trim();
    (!label.is_empty() && !label.contains('%')).then(|| label.to_owned())
}

/// Texte de la pilule pendant une copie : le pourcentage d'abord (la suite
/// peut être tronquée).
pub fn copy_text(percent: Option<u8>, label: Option<&str>) -> String {
    let label = label.map_or_else(
        || tr!("File operation", "Opération sur les fichiers"),
        str::to_owned,
    );
    match percent {
        Some(p) => format!("{p} % · {label}"),
        None => label,
    }
}

/// Une copie qui disparaît au-delà de ce seuil est terminée ; en deçà, elle a
/// été annulée (la fenêtre ne passe pas toujours par « 100% »).
pub const DONE_PERCENT: u8 = 95;

// --- Téléchargements -----------------------------------------------------------

/// Extensions des fichiers que les navigateurs écrivent pendant un téléchargement.
const PARTIAL_EXTENSIONS: [&str; 5] = ["crdownload", "part", "download", "opdownload", "partial"];

/// Nom définitif d'un fichier partiel : `video.mp4.crdownload` donne `video.mp4`.
pub fn partial_target(file_name: &str) -> Option<String> {
    let (stem, ext) = file_name.rsplit_once('.')?;
    (!stem.is_empty()
        && PARTIAL_EXTENSIONS
            .iter()
            .any(|e| ext.eq_ignore_ascii_case(e)))
    .then(|| stem.to_owned())
}

/// Nom à montrer : Chrome écrit d'abord « Unconfirmed 123456 », sans intérêt.
fn display_name(target: &str) -> String {
    if target.starts_with("Unconfirmed ") {
        tr!("Download", "Téléchargement")
    } else {
        target.to_owned()
    }
}

/// « 128 MB » (« 128 Mo ») : unités décimales, une décimale sous 10.
pub fn format_size(bytes: u64) -> String {
    let steps = match bw_i18n::lang() {
        Lang::Fr => ["o", "Ko", "Mo", "Go", "To"],
        Lang::En => ["B", "KB", "MB", "GB", "TB"],
    };
    let mut value = bytes as f64;
    let mut step = 0;
    while value >= 999.5 && step + 1 < steps.len() {
        value /= 1000.0;
        step += 1;
    }
    let number = if step == 0 || value >= 9.95 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    };
    let number = match bw_i18n::lang() {
        Lang::Fr => number.replace('.', ","),
        Lang::En => number,
    };
    format!("{number} {}", steps[step])
}

/// Un fichier partiel qui n'a pas été écrit depuis aussi longtemps n'est pas un
/// téléchargement en cours : abandon, pause, reste d'un navigateur fermé en
/// plein élan (Firefox laisse des `.part`). Il reprend dès qu'il grossit.
pub const STALE_SECS: u64 = 30;

/// Un fichier partiel trouvé dans le dossier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partial {
    pub path: PathBuf,
    pub size: u64,
    /// Secondes depuis la dernière écriture.
    pub idle_secs: u64,
}

/// Suit les téléchargements en cours d'un relevé à l'autre.
#[derive(Debug, Default)]
pub struct DownloadTracker {
    files: HashMap<PathBuf, u64>,
    /// Vitesse lissée (octets par seconde).
    rate: f64,
}

impl DownloadTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Prend un relevé : `now` liste les partiels présents, `elapsed` donne les
    /// secondes depuis le relevé précédent, `exists` dit si un fichier existe.
    ///
    /// Retourne les noms des téléchargements **terminés** depuis le dernier
    /// relevé : un partiel disparu dont le fichier définitif existe. Un partiel
    /// disparu sans fichier définitif est un téléchargement annulé.
    pub fn update(
        &mut self,
        now: &[Partial],
        elapsed: f64,
        exists: impl Fn(&Path) -> bool,
    ) -> Vec<String> {
        // Seuls comptent les partiels écrits récemment.
        let live: Vec<&Partial> = now.iter().filter(|p| p.idle_secs <= STALE_SECS).collect();
        let grown: u64 = live
            .iter()
            .filter_map(|p| {
                self.files
                    .get(&p.path)
                    .map(|before| p.size.saturating_sub(*before))
            })
            .sum();
        let instant = grown as f64 / elapsed.max(0.1);
        self.rate = if self.rate == 0.0 {
            instant
        } else {
            self.rate * 0.5 + instant * 0.5
        };

        let mut done = Vec::new();
        for path in self.files.keys() {
            // Encore là (même immobile) : ni terminé ni annulé.
            if now.iter().any(|p| &p.path == path) {
                continue;
            }
            let Some(target) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(partial_target)
            else {
                continue;
            };
            if exists(&path.with_file_name(&target)) && !target.starts_with("Unconfirmed ") {
                done.push(target);
            }
        }
        done.sort();

        self.files = live.iter().map(|p| (p.path.clone(), p.size)).collect();
        if self.files.is_empty() {
            self.rate = 0.0;
        }
        done
    }

    /// Au moins un téléchargement est en cours.
    pub fn active(&self) -> bool {
        !self.files.is_empty()
    }

    /// Octets reçus, tous téléchargements confondus.
    pub fn received(&self) -> u64 {
        self.files.values().sum()
    }

    /// Vitesse (octets par seconde).
    pub fn rate(&self) -> u64 {
        self.rate as u64
    }

    /// « video.mp4 · 128 Mo · 3,2 Mo/s » : texte de la pilule.
    pub fn summary(&self) -> Option<String> {
        if self.files.is_empty() {
            return None;
        }
        let head = if self.files.len() == 1 {
            let path = self.files.keys().next()?;
            let name = path.file_name()?.to_str()?;
            display_name(&partial_target(name).unwrap_or_else(|| name.to_owned()))
        } else {
            let count = self.files.len();
            tr!("{count} downloads", "{count} téléchargements")
        };
        let size = format_size(self.received());
        Some(if self.rate() >= 1000 {
            format!("{head} · {size} · {}/s", format_size(self.rate()))
        } else {
            format!("{head} · {size}")
        })
    }
}

/// « Téléchargé : video.mp4 ».
pub fn download_done(name: &str) -> String {
    tr!("Downloaded: {name}", "Téléchargé : {name}")
}

/// Fin d'une copie, d'un déplacement ou d'une suppression.
pub fn copy_done() -> String {
    tr!("File operation finished", "Opération terminée")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La langue est globale au processus : les tests qui en dépendent se
    /// passent le relais au lieu de se la disputer.
    static LANG: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lang(lang: Lang) -> std::sync::MutexGuard<'static, ()> {
        let guard = LANG.lock().unwrap_or_else(|e| e.into_inner());
        bw_i18n::set(lang);
        guard
    }

    fn partial(name: &str, size: u64) -> Partial {
        Partial {
            path: PathBuf::from("dl").join(name),
            size,
            idle_secs: 0,
        }
    }

    fn stale(name: &str, size: u64) -> Partial {
        Partial {
            idle_secs: STALE_SECS + 1,
            ..partial(name, size)
        }
    }

    #[test]
    fn percent_from_window_titles() {
        assert_eq!(parse_percent("17% terminé"), Some(17));
        assert_eq!(parse_percent("En pause - 17% terminé"), Some(17));
        assert_eq!(parse_percent("42 % complete"), Some(42));
        assert_eq!(parse_percent("100% complete"), Some(100));
        assert_eq!(parse_percent("0% terminé"), Some(0));
        assert_eq!(parse_percent("Calcul du temps restant"), None);
        assert_eq!(parse_percent("250% bizarre"), None);
        assert_eq!(parse_percent("% sans chiffre"), None);
    }

    #[test]
    fn operation_labels() {
        assert_eq!(
            operation_label("Copie d’un élément de copysrc vers copydst2 - En pause - 17% terminé")
                .as_deref(),
            Some("Copie d’un élément de copysrc vers copydst2")
        );
        assert_eq!(operation_label("17% terminé"), None);
        assert_eq!(operation_label(""), None);
    }

    #[test]
    fn copy_texts() {
        let _lang = lang(Lang::En);
        assert_eq!(
            copy_text(Some(17), Some("Copying 2 items")),
            "17 % · Copying 2 items"
        );
        assert_eq!(copy_text(None, None), "File operation");
        assert_eq!(copy_text(Some(5), None), "5 % · File operation");
        assert_eq!(copy_done(), "File operation finished");
    }

    #[test]
    fn partial_files() {
        assert_eq!(
            partial_target("video.mp4.crdownload").as_deref(),
            Some("video.mp4")
        );
        assert_eq!(
            partial_target("archive.zip.part").as_deref(),
            Some("archive.zip")
        );
        assert_eq!(
            partial_target("Setup.exe.PARTIAL").as_deref(),
            Some("Setup.exe")
        );
        assert_eq!(partial_target("video.mp4"), None);
        assert_eq!(partial_target(".part"), None);
        assert_eq!(partial_target("notes.txt"), None);
    }

    #[test]
    fn sizes() {
        let _lang = lang(Lang::En);
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2_300_000), "2.3 MB");
        assert_eq!(format_size(128_000_000), "128 MB");
        assert_eq!(format_size(3_400_000_000), "3.4 GB");
        bw_i18n::set(Lang::Fr);
        assert_eq!(format_size(2_300_000), "2,3 Mo");
    }

    #[test]
    fn tracks_a_download_to_completion() {
        let _lang = lang(Lang::En);
        let mut t = DownloadTracker::new();
        assert!(!t.active());
        assert_eq!(t.summary(), None);

        // Premier relevé : on voit le fichier, sans vitesse encore.
        let done = t.update(&[partial("video.mp4.crdownload", 1_000_000)], 1.0, |_| {
            false
        });
        assert!(done.is_empty() && t.active());
        assert_eq!(t.summary().as_deref(), Some("video.mp4 · 1.0 MB"));

        // Il grossit de 2 Mo en une seconde.
        t.update(&[partial("video.mp4.crdownload", 3_000_000)], 1.0, |_| {
            false
        });
        assert_eq!(t.rate(), 2_000_000);
        assert_eq!(
            t.summary().as_deref(),
            Some("video.mp4 · 3.0 MB · 2.0 MB/s")
        );

        // Le partiel disparaît et le fichier définitif existe : terminé.
        let done = t.update(&[], 1.0, |p| p == Path::new("dl").join("video.mp4"));
        assert_eq!(done, ["video.mp4"]);
        assert!(!t.active());
        assert_eq!(t.rate(), 0);
    }

    #[test]
    fn cancelled_and_unconfirmed_downloads_are_not_announced() {
        let _lang = lang(Lang::En);
        let mut t = DownloadTracker::new();
        t.update(&[partial("a.zip.part", 10)], 1.0, |_| false);
        // Le partiel disparaît sans fichier définitif : annulé.
        assert!(t.update(&[], 1.0, |_| false).is_empty());

        t.update(&[partial("Unconfirmed 123.crdownload", 10)], 1.0, |_| false);
        assert_eq!(t.summary().as_deref(), Some("Download · 10 B"));
        // Même si un fichier du même nom existait, son nom ne dirait rien.
        assert!(t.update(&[], 1.0, |_| true).is_empty());
    }

    #[test]
    fn abandoned_partials_are_not_downloads() {
        let _lang = lang(Lang::En);
        let mut t = DownloadTracker::new();
        // Un vieux `.part` laissé dans le dossier : rien à suivre.
        let done = t.update(&[stale("old.xlsx.part", 36_000)], 1.0, |_| true);
        assert!(done.is_empty() && !t.active());
        assert_eq!(t.summary(), None);

        // Il se remet à grossir : c'est un téléchargement.
        t.update(&[partial("old.xlsx.part", 40_000)], 1.0, |_| false);
        assert!(t.active());
        // Il cale : on ne l'affiche plus, sans annoncer de fin.
        let done = t.update(&[stale("old.xlsx.part", 40_000)], 1.0, |_| true);
        assert!(done.is_empty() && !t.active());
        // Il disparaît ensuite : plus suivi, donc rien à annoncer non plus.
        assert!(t.update(&[], 1.0, |_| true).is_empty());
    }

    #[test]
    fn several_downloads_are_summed() {
        let _lang = lang(Lang::En);
        let mut t = DownloadTracker::new();
        t.update(
            &[
                partial("a.zip.part", 1_000_000),
                partial("b.iso.crdownload", 4_000_000),
            ],
            1.0,
            |_| false,
        );
        assert_eq!(t.summary().as_deref(), Some("2 downloads · 5.0 MB"));
        assert_eq!(download_done("a.zip"), "Downloaded: a.zip");
    }
}
