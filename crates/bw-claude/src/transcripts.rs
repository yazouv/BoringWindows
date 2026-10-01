//! Lecture des transcripts de Claude Code (`~/.claude/projects/<projet>/<session>.jsonl`) :
//! sessions récentes (titre, dossier, dernière activité) et consommation de tokens.
//!
//! La consommation est une **estimation locale** : les limites de l'abonnement
//! sont côté serveur et ne sont pas lisibles. Un même message apparaît sur
//! plusieurs lignes (un bloc de contenu par ligne) avec le même `message.id` : on
//! ne le compte qu'une fois.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Duration, Utc};
use serde_json::Value;

/// Tokens d'un message de l'assistant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
}

impl Tokens {
    /// Total retenu pour l'estimation : entrée, sortie, création de cache ;
    /// la lecture de cache (quasi gratuite) seulement si demandé.
    pub fn total(&self, count_cache_reads: bool) -> u64 {
        self.input
            + self.output
            + self.cache_creation
            + if count_cache_reads {
                self.cache_read
            } else {
                0
            }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageEntry {
    pub at: DateTime<Utc>,
    pub tokens: Tokens,
}

/// Une conversation retrouvée dans un transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: String,
    /// Dossier de travail de la session (là où la reprendre).
    pub cwd: PathBuf,
    pub title: String,
    pub last_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileScan {
    pub session: Option<SessionInfo>,
    pub usage: Vec<UsageEntry>,
}

/// Résumé de la fenêtre de consommation en cours (5 h par défaut).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageSummary {
    /// Fin de la fenêtre active (`None` : aucune fenêtre ouverte).
    pub window_end: Option<DateTime<Utc>>,
    pub tokens: u64,
    pub messages: u32,
}

/// Identifiant de session valide (UUID) : il finira dans une ligne de commande.
pub fn is_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

const MAX_TITLE_CHARS: usize = 80;

fn parse_time(v: &Value) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(v.as_str()?)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Valeur de `"timestamp":"…"` en tête de ligne, sans analyser le JSON.
fn quick_timestamp(line: &str) -> Option<DateTime<Utc>> {
    const KEY: &str = "\"timestamp\":\"";
    let start = line.find(KEY)? + KEY.len();
    let end = line[start..].find('"')?;
    DateTime::parse_from_rfc3339(&line[start..start + end])
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn clip(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = one_line.chars().take(MAX_TITLE_CHARS).collect();
    if one_line.chars().count() > MAX_TITLE_CHARS {
        out.push('…');
    }
    out
}

/// Texte d'un vrai message de l'utilisateur (pas un résultat d'outil ni une balise système).
fn prompt_text(message: &Value) -> Option<String> {
    let content = message.get("content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .find_map(|p| (p.get("type")?.as_str()? == "text").then(|| p.get("text")?.as_str()))
            .flatten()?
            .to_owned(),
        _ => return None,
    };
    let trimmed = text.trim();
    (!trimmed.is_empty() && !trimmed.starts_with('<') && !trimmed.starts_with('['))
        .then(|| clip(trimmed))
}

/// Lit un transcript. Les lignes illisibles sont ignorées.
pub fn scan_file(path: &Path) -> std::io::Result<FileScan> {
    let id = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let file = std::fs::File::open(path)?;

    let mut cwd: Option<PathBuf> = None;
    let mut custom_title: Option<String> = None;
    let mut first_prompt: Option<String> = None;
    let mut last_at: Option<DateTime<Utc>> = None;
    // Dernière valeur vue pour chaque message (les blocs d'un message se répètent).
    let mut messages: HashMap<String, UsageEntry> = HashMap::new();

    for line in BufReader::new(file).lines() {
        let Ok(line) = line else { continue };
        // Horodatage sans analyser tout le JSON : la plupart des lignes (résultats
        // d'outils, pièces jointes) ne servent qu'à la date de dernière activité.
        if let Some(at) = quick_timestamp(&line) {
            last_at = Some(last_at.map_or(at, |l| l.max(at)));
        }
        let useful = line.contains("\"custom-title\"")
            || (first_prompt.is_none() && line.contains("\"type\":\"user\""))
            || (cwd.is_none() && line.contains("\"cwd\""))
            || (line.contains("\"usage\"") && line.contains("\"assistant\""));
        if !useful {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if cwd.is_none()
            && let Some(c) = v.get("cwd").and_then(Value::as_str)
        {
            cwd = Some(PathBuf::from(c));
        }
        match v.get("type").and_then(Value::as_str) {
            Some("custom-title") => {
                if let Some(t) = v.get("customTitle").and_then(Value::as_str) {
                    custom_title = Some(clip(t));
                }
            }
            Some("user") if first_prompt.is_none() => {
                if v.get("isSidechain").and_then(Value::as_bool) != Some(true)
                    && let Some(text) = v.get("message").and_then(prompt_text)
                {
                    first_prompt = Some(text);
                }
            }
            Some("assistant") => {
                let Some(message) = v.get("message") else {
                    continue;
                };
                let (Some(usage), Some(mid), Some(at)) = (
                    message.get("usage"),
                    message.get("id").and_then(Value::as_str),
                    v.get("timestamp").and_then(parse_time),
                ) else {
                    continue;
                };
                if message.get("model").and_then(Value::as_str) == Some("<synthetic>") {
                    continue;
                }
                let n = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
                messages.insert(
                    mid.to_owned(),
                    UsageEntry {
                        at,
                        tokens: Tokens {
                            input: n("input_tokens"),
                            output: n("output_tokens"),
                            cache_creation: n("cache_creation_input_tokens"),
                            cache_read: n("cache_read_input_tokens"),
                        },
                    },
                );
            }
            _ => {}
        }
    }

    let mut usage: Vec<UsageEntry> = messages.into_values().collect();
    usage.sort_by_key(|e| e.at);
    let session = match (is_session_id(&id), cwd, last_at) {
        (true, Some(cwd), Some(last_at)) => Some(SessionInfo {
            id,
            cwd,
            title: custom_title.or(first_prompt).unwrap_or_default(),
            last_at,
        }),
        _ => None,
    };
    Ok(FileScan { session, usage })
}

/// Transcripts des projets (`<dir>/<projet>/<session>.jsonl`), du plus récent au plus ancien.
pub fn list_transcripts(projects_dir: &Path) -> Vec<(PathBuf, SystemTime, u64)> {
    let mut files = Vec::new();
    let Ok(projects) = std::fs::read_dir(projects_dir) else {
        return files;
    };
    for project in projects.flatten() {
        let Ok(entries) = std::fs::read_dir(project.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|x| x == "jsonl")
                && let Ok(meta) = entry.metadata()
                && meta.is_file()
                && let Ok(modified) = meta.modified()
            {
                files.push((path, modified, meta.len()));
            }
        }
    }
    files.sort_by_key(|(_, m, _)| std::cmp::Reverse(*m));
    files
}

/// Consommation de la fenêtre active. Une fenêtre commence à l'heure exacte du
/// premier message qui suit la fin de la précédente et dure `window`.
///
/// Les fenêtres de l'abonnement sont communes à tout le compte (claude.ai,
/// application, Claude Code) : on n'en voit qu'une partie. `reset_at`, la fin
/// d'une fenêtre connue (lue dans `/usage`), recale le calcul : la fenêtre
/// `[reset_at - window, reset_at)` est prise comme point de départ, et les
/// messages d'avant ne comptent pas.
pub fn summarize(
    entries: &[UsageEntry],
    now: DateTime<Utc>,
    window: Duration,
    count_cache_reads: bool,
    reset_at: Option<DateTime<Utc>>,
) -> UsageSummary {
    let mut sorted: Vec<&UsageEntry> = entries.iter().filter(|e| e.at <= now).collect();
    sorted.sort_by_key(|e| e.at);

    // (début, fin) de la fenêtre en cours de lecture.
    let mut block: Option<(DateTime<Utc>, DateTime<Utc>)> = reset_at.map(|end| (end - window, end));
    let mut tokens = 0u64;
    let mut messages = 0u32;
    for e in sorted {
        // Avant la fenêtre connue : message d'une fenêtre précédente.
        if reset_at.is_some_and(|end| e.at < end - window) {
            continue;
        }
        if block.is_none_or(|(_, end)| e.at >= end) {
            block = Some((e.at, e.at + window));
            tokens = 0;
            messages = 0;
        }
        tokens += e.tokens.total(count_cache_reads);
        messages += 1;
    }
    match block {
        Some((_, end)) if now < end => UsageSummary {
            window_end: Some(end),
            tokens,
            messages,
        },
        _ => UsageSummary::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 1, h, m, 0).unwrap()
    }

    fn entry(h: u32, m: u32, output: u64) -> UsageEntry {
        UsageEntry {
            at: at(h, m),
            tokens: Tokens {
                input: 10,
                output,
                cache_creation: 100,
                cache_read: 1000,
            },
        }
    }

    fn write(dir: &Path, name: &str, lines: &[&str]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, lines.join("\n")).unwrap();
        path
    }

    const SID: &str = "5bc9e738-a886-40fd-9611-8e80daf3b9e8";

    fn assistant(id: &str, ts: &str, output: u64) -> String {
        format!(
            r#"{{"type":"assistant","isSidechain":false,"timestamp":"{ts}","message":{{"id":"{id}","role":"assistant","model":"claude-x","content":[{{"type":"text","text":"ok"}}],"usage":{{"input_tokens":10,"cache_creation_input_tokens":100,"cache_read_input_tokens":1000,"output_tokens":{output}}}}}}}"#
        )
    }

    #[test]
    fn scans_title_cwd_and_dedupes_messages() {
        let dir = tempfile::tempdir().unwrap();
        let user = r#"{"type":"user","isSidechain":false,"timestamp":"2026-10-01T18:00:00.000Z","cwd":"D:\\Projet","sessionId":"x","message":{"role":"user","content":"Corrige le bug du  chargement"}}"#;
        let a1 = assistant("m1", "2026-10-01T18:00:05.000Z", 20);
        // Même message répété (bloc suivant) avec l'usage final.
        let a2 = assistant("m1", "2026-10-01T18:00:06.000Z", 50);
        let a3 = assistant("m2", "2026-10-01T18:05:00.000Z", 30);
        let synthetic = a3.replace("claude-x", "<synthetic>").replace("m2", "m3");
        let title = format!(
            r#"{{"type":"custom-title","customTitle":"Bug du chargement","sessionId":"{SID}"}}"#
        );
        let path = write(
            dir.path(),
            &format!("{SID}.jsonl"),
            &[user, &a1, &a2, &a3, &synthetic, "pas du json", &title],
        );

        let scan = scan_file(&path).unwrap();
        let s = scan.session.unwrap();
        assert_eq!(s.id, SID);
        assert_eq!(s.title, "Bug du chargement");
        assert_eq!(s.cwd, PathBuf::from(r"D:\Projet"));
        assert_eq!(s.last_at, at(18, 5));
        // m1 compté une fois (dernière valeur), m3 « synthetic » ignoré.
        assert_eq!(scan.usage.len(), 2);
        assert_eq!(scan.usage[0].tokens.output, 50);
        assert_eq!(scan.usage[1].tokens.output, 30);
    }

    #[test]
    fn title_falls_back_to_first_real_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let caveat = r#"{"type":"user","timestamp":"2026-10-01T18:00:00Z","cwd":"/p","message":{"role":"user","content":"<local-command-caveat>x</local-command-caveat>"}}"#;
        let tool = r#"{"type":"user","timestamp":"2026-10-01T18:00:01Z","message":{"role":"user","content":[{"type":"tool_result","content":"sortie"}]}}"#;
        let real = r#"{"type":"user","timestamp":"2026-10-01T18:00:02Z","message":{"role":"user","content":[{"type":"text","text":"Ajoute une étagère"}]}}"#;
        let path = write(dir.path(), &format!("{SID}.jsonl"), &[caveat, tool, real]);
        let s = scan_file(&path).unwrap().session.unwrap();
        assert_eq!(s.title, "Ajoute une étagère");

        let long = "mot ".repeat(60);
        let long_line = format!(
            r#"{{"type":"user","timestamp":"2026-10-01T18:00:02Z","cwd":"/p","message":{{"role":"user","content":"{long}"}}}}"#
        );
        let path = write(dir.path(), &format!("{SID}.jsonl"), &[&long_line]);
        let t = scan_file(&path).unwrap().session.unwrap().title;
        assert!(
            t.chars().count() <= MAX_TITLE_CHARS + 1 && t.ends_with('…'),
            "{t}"
        );
    }

    #[test]
    fn rejects_unsafe_session_ids() {
        assert!(is_session_id(SID));
        for bad in ["", "a b", "x;rm", "../x", "a&b"] {
            assert!(!is_session_id(bad), "{bad}");
        }
        let dir = tempfile::tempdir().unwrap();
        let line = r#"{"type":"user","timestamp":"2026-10-01T18:00:00Z","cwd":"/p","message":{"role":"user","content":"x"}}"#;
        let path = write(dir.path(), "pas un id.jsonl", &[line]);
        assert!(scan_file(&path).unwrap().session.is_none());
    }

    #[test]
    fn summary_counts_the_active_window_only() {
        let w = Duration::hours(5);
        let entries = [
            entry(8, 10, 100),
            entry(8, 50, 100),
            entry(14, 5, 200),
            entry(15, 0, 300),
        ];
        // Fenêtre commencée à l'heure exacte du premier message (14:05).
        let s = summarize(&entries, at(16, 0), w, false, None);
        assert_eq!(
            (s.messages, s.tokens),
            (2, (10 + 200 + 100) + (10 + 300 + 100))
        );
        assert_eq!(s.window_end, Some(at(19, 5)));
        // Avec la lecture de cache, plus de tokens.
        assert!(summarize(&entries, at(16, 0), w, true, None).tokens > s.tokens);
        // Fenêtre expirée : plus rien d'actif.
        assert_eq!(
            summarize(&entries, at(21, 0), w, false, None),
            UsageSummary::default()
        );
        // Premier bloc seul, encore actif à 10:00.
        let early = summarize(&entries, at(10, 0), w, false, None);
        assert_eq!((early.messages, early.window_end), (2, Some(at(13, 10))));
        assert_eq!(
            summarize(&[], at(10, 0), w, false, None),
            UsageSummary::default()
        );
    }

    #[test]
    fn known_reset_time_recalibrates_the_window() {
        let w = Duration::hours(5);
        // Messages continus depuis 18:26 ; le vrai reset (vu dans /usage) est à 01:01
        // le lendemain, donc la fenêtre réelle a commencé à 20:01.
        let entries = [
            entry(18, 26, 100),
            entry(19, 40, 100),
            entry(20, 5, 200),
            entry(20, 10, 300),
        ];
        let now = at(20, 15);
        // Sans calage : fenêtre comptée depuis 18:26 → fin à 23:26.
        let blind = summarize(&entries, now, w, false, None);
        assert_eq!(blind.window_end, Some(at(23, 26)));
        // Calée sur le reset de 01:01 : seuls les messages depuis 20:01 comptent.
        let reset = Utc.with_ymd_and_hms(2026, 10, 2, 1, 1, 0).unwrap();
        let s = summarize(&entries, now, w, false, Some(reset));
        assert_eq!(s.window_end, Some(reset));
        assert_eq!(
            (s.messages, s.tokens),
            (2, (10 + 200 + 100) + (10 + 300 + 100))
        );
        // Fenêtre calée sans message dedans : active, vide.
        let empty = summarize(&entries[..2], now, w, false, Some(reset));
        assert_eq!(
            (empty.window_end, empty.messages, empty.tokens),
            (Some(reset), 0, 0)
        );
        // Le reset passé : on repart des messages qui suivent, début exact.
        let later = [UsageEntry {
            at: Utc.with_ymd_and_hms(2026, 10, 2, 1, 30, 0).unwrap(),
            tokens: entry(0, 0, 50).tokens,
        }];
        let after = summarize(
            &later,
            Utc.with_ymd_and_hms(2026, 10, 2, 1, 40, 0).unwrap(),
            w,
            false,
            Some(reset),
        );
        assert_eq!(
            after.window_end,
            Some(Utc.with_ymd_and_hms(2026, 10, 2, 6, 30, 0).unwrap())
        );
        // Reset périmé et plus aucun message : rien d'actif.
        assert_eq!(
            summarize(
                &entries,
                Utc.with_ymd_and_hms(2026, 10, 2, 3, 0, 0).unwrap(),
                w,
                false,
                Some(reset)
            ),
            UsageSummary::default()
        );
    }

    #[test]
    fn lists_transcripts_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("proj-a");
        let p2 = dir.path().join("proj-b");
        std::fs::create_dir_all(&p1).unwrap();
        std::fs::create_dir_all(&p2).unwrap();
        write(&p1, "old.jsonl", &["{}"]);
        std::thread::sleep(std::time::Duration::from_millis(30));
        write(&p2, "new.jsonl", &["{}"]);
        write(&p2, "notes.txt", &["x"]);
        let files = list_transcripts(dir.path());
        let names: Vec<_> = files
            .iter()
            .map(|(p, _, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["new.jsonl", "old.jsonl"]);
        assert!(list_transcripts(&dir.path().join("absent")).is_empty());
    }
}
