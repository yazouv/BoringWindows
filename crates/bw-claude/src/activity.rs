//! Activité Claude Code lue dans les transcripts locaux : conversations
//! récentes et estimation de la consommation de la fenêtre en cours.
//!
//! Rien ne tourne en arrière-plan : le module relit les transcripts au
//! démarrage, puis à la demande (ouverture de l'île), au plus toutes les 15 s,
//! et seulement les fichiers qui ont changé.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant, SystemTime};

use bw_core::{Module, ModuleCtx};
use bw_i18n::tr;
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use tokio::sync::mpsc::unbounded_channel;

use crate::transcripts::{
    FileScan, UsageEntry, UsageSummary, list_transcripts, scan_file, summarize,
};

pub const ACTIVITY_ID: &str = "claude_activity";

/// Intervalle minimal entre deux lectures des transcripts.
const MIN_INTERVAL: StdDuration = StdDuration::from_secs(15);

/// Section `[modules.claude_activity]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ActivityConfig {
    pub enabled: bool,
    /// Nombre de conversations récentes proposées (0 : aucune).
    pub recent: usize,
    /// Durée de la fenêtre de consommation, en heures (5 pour Claude).
    pub window_hours: u32,
    /// Limite estimée de la fenêtre, en tokens : sert à dessiner la jauge (0 : pas de jauge).
    pub limit_tokens: u64,
    /// Compter aussi les lectures de cache (très nombreuses, quasi gratuites).
    pub count_cache_reads: bool,
    /// Fin d'une fenêtre connue (RFC 3339, lue dans `/usage`) : recale le calcul.
    pub reset_at: String,
    /// Dossier des transcripts ; vide = `<dossier de Claude Code>/projects`.
    pub projects_dir: String,
}

impl Default for ActivityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            recent: 4,
            window_hours: 5,
            limit_tokens: 0,
            count_cache_reads: false,
            reset_at: String::new(),
            projects_dir: String::new(),
        }
    }
}

impl ActivityConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            config.recent <= 4,
            tr!(
                "modules.claude_activity.recent must be between 0 and 4",
                "modules.claude_activity.recent doit être entre 0 et 4"
            )
        );
        anyhow::ensure!(
            (1..=24).contains(&config.window_hours),
            tr!(
                "modules.claude_activity.window_hours must be between 1 and 24",
                "modules.claude_activity.window_hours doit être entre 1 et 24"
            )
        );
        anyhow::ensure!(
            config.limit_tokens <= 1_000_000_000_000,
            tr!(
                "modules.claude_activity.limit_tokens is too large",
                "modules.claude_activity.limit_tokens est trop grand"
            )
        );
        anyhow::ensure!(
            config.reset_at.trim().is_empty() || config.reset_at_utc().is_some(),
            tr!(
                "modules.claude_activity.reset_at must be a date like 2026-10-02T01:01:00Z",
                "modules.claude_activity.reset_at doit être une date comme 2026-10-02T01:01:00Z"
            )
        );
        Ok(config)
    }

    /// Fin de fenêtre connue, si renseignée.
    pub fn reset_at_utc(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(self.reset_at.trim())
            .ok()
            .map(|d| d.with_timezone(&Utc))
    }

    pub fn projects_path(&self) -> PathBuf {
        if self.projects_dir.trim().is_empty() {
            crate::install::claude_dir().join("projects")
        } else {
            PathBuf::from(self.projects_dir.trim())
        }
    }

    fn window(&self) -> Duration {
        Duration::hours(i64::from(self.window_hours))
    }
}

/// Une conversation qu'on peut rouvrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentSession {
    pub id: String,
    pub cwd: PathBuf,
    pub title: String,
    /// Nom du dossier de travail (dernier élément du chemin).
    pub project: String,
    pub last_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivitySnapshot {
    pub usage: UsageSummary,
    pub limit_tokens: u64,
    pub recent: Vec<RecentSession>,
}

#[derive(Default)]
pub struct Cache {
    files: HashMap<PathBuf, (SystemTime, u64, Arc<FileScan>)>,
}

impl Cache {
    fn scan(&mut self, path: &PathBuf, modified: SystemTime, len: u64) -> Option<Arc<FileScan>> {
        if let Some((m, l, scan)) = self.files.get(path)
            && *m == modified
            && *l == len
        {
            return Some(scan.clone());
        }
        let scan = Arc::new(scan_file(path).ok()?);
        self.files
            .insert(path.clone(), (modified, len, scan.clone()));
        Some(scan)
    }
}

/// Lit les transcripts utiles (récents) et construit l'état affiché.
pub fn collect(config: &ActivityConfig, cache: &mut Cache, now: DateTime<Utc>) -> ActivitySnapshot {
    let files = list_transcripts(&config.projects_path());
    cache
        .files
        .retain(|p, _| files.iter().any(|(f, _, _)| f == p));

    // Seuls les fichiers modifiés dans la fenêtre peuvent contenir des messages actifs.
    let window_start = SystemTime::from(now - config.window() - Duration::hours(1));
    let mut usage: Vec<UsageEntry> = Vec::new();
    let mut sessions = Vec::new();
    let mut kept_recent = 0;
    for (path, modified, len) in &files {
        let in_window = *modified >= window_start;
        // Un peu plus de fichiers que demandé : certains n'ont pas de session lisible.
        let wants_recent = kept_recent < config.recent.saturating_mul(2) + 2;
        if !in_window && !wants_recent {
            break;
        }
        let Some(scan) = cache.scan(path, *modified, *len) else {
            continue;
        };
        if in_window {
            usage.extend(scan.usage.iter().cloned());
        }
        // Seules les sessions rouvrables ici : une session distante (SSH) a un dossier inexistant en local.
        if let Some(session) = scan.session.as_ref().filter(|s| s.cwd.is_dir()) {
            kept_recent += 1;
            sessions.push(session.clone());
        }
    }

    sessions.sort_by_key(|s| std::cmp::Reverse(s.last_at));
    let recent = sessions
        .into_iter()
        .take(config.recent)
        .map(|s| RecentSession {
            project: s.cwd.file_name().map_or_else(
                || s.cwd.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            id: s.id,
            title: s.title,
            cwd: s.cwd,
            last_at: s.last_at,
        })
        .collect();

    ActivitySnapshot {
        usage: summarize(
            &usage,
            now,
            config.window(),
            config.count_cache_reads,
            config.reset_at_utc(),
        ),
        limit_tokens: config.limit_tokens,
        recent,
    }
}

pub struct ActivityModule {
    config: ActivityConfig,
    refresh: Option<tokio::sync::mpsc::UnboundedSender<()>>,
}

impl ActivityModule {
    pub fn new(config: ActivityConfig) -> Self {
        Self {
            config,
            refresh: None,
        }
    }
}

impl Module for ActivityModule {
    fn id(&self) -> &'static str {
        ACTIVITY_ID
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let (tx, mut rx) = unbounded_channel::<()>();
        self.refresh = Some(tx);
        let config = self.config.clone();
        ctx.clone().spawn(async move {
            let mut cache = Cache::default();
            loop {
                let cfg = config.clone();
                let run = tokio::task::spawn_blocking(move || {
                    let snapshot = collect(&cfg, &mut cache, Utc::now());
                    (snapshot, cache)
                })
                .await;
                let Ok((snapshot, returned)) = run else {
                    return;
                };
                cache = returned;
                ctx.set_state(snapshot);
                let started = Instant::now();

                // Attend une demande de rafraîchissement ; trop rapprochées, elles sont ignorées.
                loop {
                    match rx.recv().await {
                        None => return,
                        Some(()) if started.elapsed() >= MIN_INTERVAL => break,
                        Some(()) => {}
                    }
                }
            }
        });
        Ok(())
    }

    /// Action : `refresh` (relire les transcripts, ex. à l'ouverture de l'île).
    fn on_action(&mut self, action: &str) {
        if action == "refresh"
            && let Some(tx) = &self.refresh
        {
            let _ = tx.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    const A: &str = "11111111-1111-1111-1111-111111111111";
    const B: &str = "22222222-2222-2222-2222-222222222222";
    const C: &str = "33333333-3333-3333-3333-333333333333";

    fn session(dir: &std::path::Path, project: &str, id: &str, cwd: &str, ts: &str, title: &str) {
        let p = dir.join(project);
        std::fs::create_dir_all(&p).unwrap();
        let user = format!(
            r#"{{"type":"user","timestamp":"{ts}","cwd":"{cwd}","message":{{"role":"user","content":"{title}"}}}}"#
        );
        let assistant = format!(
            r#"{{"type":"assistant","timestamp":"{ts}","message":{{"id":"m-{id}","role":"assistant","model":"x","usage":{{"input_tokens":10,"output_tokens":40,"cache_creation_input_tokens":50,"cache_read_input_tokens":9999}}}}}}"#
        );
        std::fs::write(
            p.join(format!("{id}.jsonl")),
            format!("{user}\n{assistant}\n"),
        )
        .unwrap();
    }

    #[test]
    fn collects_recent_sessions_and_usage() {
        let dir = tempfile::tempdir().unwrap();
        // Heure réelle : la fenêtre se calcule depuis maintenant.
        let now = Utc::now();
        let t = |m: i64| (now - Duration::minutes(m)).to_rfc3339();
        let work = tempfile::tempdir().unwrap();
        let cwd = |name: &str| {
            let p = work.path().join(name);
            std::fs::create_dir_all(&p).unwrap();
            p.to_string_lossy().replace('\\', "/")
        };
        // Session distante (SSH) : son dossier n'existe pas ici, elle n'est pas proposée.
        session(
            dir.path(),
            "ssh-x",
            C,
            "/srv/distant/app",
            &t(5),
            "Session distante",
        );
        session(
            dir.path(),
            "proj-a",
            A,
            &cwd("alpha"),
            &t(30),
            "Premier sujet",
        );
        session(
            dir.path(),
            "proj-b",
            B,
            &cwd("beta"),
            &t(10),
            "Second sujet",
        );

        let config = ActivityConfig {
            projects_dir: dir.path().display().to_string(),
            ..ActivityConfig::default()
        };
        let mut cache = Cache::default();
        let snap = collect(&config, &mut cache, now);

        assert_eq!(snap.recent.len(), 2);
        assert_eq!(snap.recent[0].id, B, "le plus récent d'abord");
        assert_eq!(snap.recent[0].project, "beta");
        assert_eq!(snap.recent[0].title, "Second sujet");
        // La session distante compte dans la consommation (c'est de l'usage), pas dans la liste.
        assert_eq!(snap.usage.messages, 3);
        assert_eq!(snap.usage.tokens, 3 * (10 + 40 + 50));
        assert!(snap.usage.window_end.is_some());
        assert_eq!(cache.files.len(), 3);

        // Seconde lecture : mêmes résultats, servis par le cache.
        assert_eq!(collect(&config, &mut cache, now), snap);

        let few = ActivityConfig {
            recent: 1,
            ..config.clone()
        };
        assert_eq!(collect(&few, &mut cache, now).recent.len(), 1);
        let none = ActivityConfig {
            recent: 0,
            ..config
        };
        assert!(collect(&none, &mut cache, now).recent.is_empty());
    }

    #[test]
    fn old_sessions_stay_listed_but_do_not_count_as_usage() {
        let dir = tempfile::tempdir().unwrap();
        let old = Utc
            .with_ymd_and_hms(2026, 1, 1, 10, 0, 0)
            .unwrap()
            .to_rfc3339();
        let work = tempfile::tempdir().unwrap();
        let cwd = work.path().to_string_lossy().replace('\\', "/");
        session(dir.path(), "p", A, &cwd, &old, "Vieux sujet");
        let config = ActivityConfig {
            projects_dir: dir.path().display().to_string(),
            ..ActivityConfig::default()
        };
        // Le fichier vient d'être écrit (mtime récent) mais ses messages sont anciens.
        let snap = collect(&config, &mut Cache::default(), Utc::now());
        assert_eq!(snap.recent.len(), 1);
        assert_eq!(snap.usage, UsageSummary::default());
    }

    #[test]
    fn config_validation() {
        assert!(ActivityConfig::from_table(None).unwrap().enabled);
        for bad in [
            "recent = 5",
            "window_hours = 0",
            "window_hours = 25",
            "limit_tokens = 2000000000000",
            "reset_at = \"demain\"",
            "recents = 3",
        ] {
            let t: toml::Table = toml::from_str(bad).unwrap();
            assert!(ActivityConfig::from_table(Some(&t)).is_err(), "{bad}");
        }
    }
}
