//! Mode présentation : savoir si tu es en appel ou en train de partager ton
//! écran, pour que l'île taise ses annonces et affiche un point « en direct ».
//!
//! Windows n'a pas d'API pour ça, mais il tient à jour, par application,
//! l'usage du micro et de la capture d'écran (registre
//! `CapabilityAccessManager\ConsentStore`, celui de l'icône de micro de la
//! barre des tâches). `live_app` lit ces entrées : pur, testable partout.

mod config;
mod module;
#[cfg(windows)]
mod win;

pub use config::PresentationConfig;
pub use module::{MODULE_ID, PresenceModule};

/// Ce que publie le module : l'application en direct, s'il y en a une.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LiveSnapshot {
    /// Nom lisible (« Discord », « Teams »…), vide si rien n'est en cours.
    pub app: String,
}

impl LiveSnapshot {
    pub fn live(&self) -> bool {
        !self.app.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Microphone,
    ScreenCapture,
}

/// Une entrée du registre : une application et son dernier usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    pub capability: Capability,
    /// Application empaquetée (nom de paquet) ou chemin de l'exécutable
    /// écrit avec des `#` à la place des `\`.
    pub app: String,
    pub packaged: bool,
    /// Instants FILETIME (100 ns depuis 1601) ; `stop == 0` : en cours.
    pub start: u64,
    pub stop: u64,
}

/// Processus en cours d'exécution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub pid: u32,
    /// Chemin complet de l'exécutable.
    pub path: String,
    /// Création du processus (FILETIME).
    pub created: u64,
}

/// Application qui utilise le micro (si c'est une appli d'appel) ou capture
/// l'écran en ce moment ; `None` sinon.
///
/// Une application qui plante laisse son entrée « en cours » pour toujours :
/// l'entrée ne compte que si l'exécutable tourne et a démarré avant le début
/// de l'usage.
/// Application en direct et, si elle n'est pas empaquetée, son processus (pour
/// savoir quand il s'arrête).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Live {
    pub app: String,
    pub pid: Option<u32>,
}

pub fn live_app(usages: &[Usage], processes: &[Process], call_apps: &[String]) -> Option<Live> {
    usages
        .iter()
        .filter(|u| u.start != 0 && u.stop == 0)
        .filter(|u| {
            u.capability == Capability::ScreenCapture || matches_any(&display_name(u), call_apps)
        })
        .find_map(|u| {
            let app = display_name(u);
            if u.packaged {
                return Some(Live { app, pid: None });
            }
            let path = u.app.replace('#', "\\");
            processes
                .iter()
                .find(|p| p.path.eq_ignore_ascii_case(&path) && p.created <= u.start)
                .map(|p| Live {
                    app,
                    pid: Some(p.pid),
                })
        })
}

/// « Discord » pour `C:#…#Discord.exe`, « MSTeams » pour
/// `MSTeams_8wekyb3d8bbwe`.
fn display_name(u: &Usage) -> String {
    if u.packaged {
        u.app.split('_').next().unwrap_or(&u.app).to_owned()
    } else {
        let file = u.app.rsplit(['#', '\\']).next().unwrap_or(&u.app);
        file.strip_suffix(".exe")
            .or_else(|| file.strip_suffix(".EXE"))
            .unwrap_or(file)
            .to_owned()
    }
}

fn matches_any(name: &str, apps: &[String]) -> bool {
    let name = name.to_lowercase();
    apps.iter()
        .any(|a| !a.trim().is_empty() && name.contains(&a.trim().to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(capability: Capability, app: &str, start: u64, stop: u64) -> Usage {
        Usage {
            capability,
            app: app.into(),
            packaged: !app.contains('#'),
            start,
            stop,
        }
    }

    fn calls() -> Vec<String> {
        PresentationConfig::default().call_apps
    }

    const DISCORD: &str = "C:#Users#me#AppData#Local#Discord#app-1.0.9#Discord.exe";

    fn discord(created: u64) -> Process {
        Process {
            pid: 7,
            path: DISCORD.replace('#', "\\"),
            created,
        }
    }

    #[test]
    fn microphone_counts_only_for_call_apps() {
        let lghub = usage(
            Capability::Microphone,
            "C:#Program Files#LGHUB#lghub_agent.exe",
            100,
            0,
        );
        let running = Process {
            pid: 3,
            path: "C:\\Program Files\\LGHUB\\lghub_agent.exe".into(),
            created: 50,
        };
        assert_eq!(live_app(&[lghub], &[running], &calls()), None);

        let call = usage(Capability::Microphone, DISCORD, 100, 0);
        assert_eq!(
            live_app(&[call], &[discord(50)], &calls())
                .map(|l| l.app)
                .as_deref(),
            Some("Discord")
        );
    }

    #[test]
    fn stale_entries_are_ignored() {
        // Terminé.
        let done = usage(Capability::ScreenCapture, DISCORD, 100, 200);
        assert_eq!(live_app(&[done], &[discord(50)], &calls()), None);
        // Resté « en cours » après un plantage : le processus n'est plus là…
        let crashed = usage(Capability::ScreenCapture, DISCORD, 100, 0);
        assert_eq!(
            live_app(std::slice::from_ref(&crashed), &[], &calls()),
            None
        );
        // … ou il a redémarré depuis.
        assert_eq!(
            live_app(std::slice::from_ref(&crashed), &[discord(150)], &calls()),
            None
        );
        assert_eq!(
            live_app(&[crashed], &[discord(50)], &calls())
                .map(|l| l.app)
                .as_deref(),
            Some("Discord")
        );
    }

    #[test]
    fn packaged_apps_are_trusted() {
        let teams = usage(Capability::Microphone, "MSTeams_8wekyb3d8bbwe", 100, 0);
        assert_eq!(
            live_app(&[teams], &[], &calls()).map(|l| l.app).as_deref(),
            Some("MSTeams")
        );
    }
}
