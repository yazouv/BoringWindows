//! `boringwindows doctor` : vérifie toute la chaîne Claude Code → île et dit
//! précisément où elle casse.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::event::HookEvent;
use crate::hook::{self, Outcome};
use crate::install::{self, Installer};
use crate::ipc::{self, Message};

/// Session fictive utilisée pour les tests (affichée « diagnostic »).
const SESSION: &str = "bw-doctor";

struct Report {
    text: String,
    problems: usize,
}

impl Report {
    fn ok(&mut self, msg: impl AsRef<str>) {
        let _ = writeln!(self.text, "  [ok]   {}", msg.as_ref());
    }

    fn bad(&mut self, msg: impl AsRef<str>) {
        self.problems += 1;
        let _ = writeln!(self.text, "  [!!]   {}", msg.as_ref());
    }

    fn info(&mut self, msg: impl AsRef<str>) {
        let _ = writeln!(self.text, "         {}", msg.as_ref());
    }

    fn section(&mut self, title: &str) {
        let _ = writeln!(self.text, "\n{title}");
    }
}

/// Lance le diagnostic et retourne le rapport. `app_exe` : l'exécutable courant.
pub fn run(app_exe: &Path) -> String {
    let mut r = Report {
        text: String::from("Diagnostic BoringWindows × Claude Code\n"),
        problems: 0,
    };
    let installer = Installer::default();
    let endpoint = ipc::endpoint();

    r.section("1. Hooks dans settings.json");
    let command = check_settings(&mut r, &installer);

    r.section("2. Relais");
    check_binary(&mut r, &installer, app_exe);

    r.section("3. BoringWindows en cours d'exécution");
    let reachable = check_app(&mut r, &endpoint);

    r.section("4. Relais lancé comme le fait Claude Code");
    if let Some(command) = &command {
        check_like_claude(&mut r, command);
    } else {
        r.info("(sauté : hooks non installés)");
    }

    r.section("5. Journal du relais (derniers appels de Claude Code)");
    check_journal(&mut r);

    if reachable {
        r.section("6. Test visuel");
        visual_test(&endpoint);
        r.info("L'île doit afficher « diagnostic · attend ta réponse » pendant 6 secondes.");
    }

    let _ = writeln!(
        r.text,
        "\n{}",
        if r.problems == 0 {
            "Aucun problème détecté. Si l'île reste vide, relance tes sessions Claude Code \
             (les hooks sont lus au démarrage) puis regarde le journal (point 5)."
                .to_owned()
        } else {
            format!(
                "{} problème(s) détecté(s), voir les lignes [!!].",
                r.problems
            )
        }
    );
    r.text
}

fn check_settings(r: &mut Report, installer: &Installer) -> Option<String> {
    let path = &installer.settings_path;
    r.info(format!("fichier : {}", path.display()));
    let settings = match installer.read() {
        Ok(s) => s,
        Err(e) => {
            r.bad(format!("lecture impossible : {e:#}"));
            return None;
        }
    };
    if !path.exists() {
        r.bad("le fichier n'existe pas : hooks jamais installés");
    }
    if settings.get("disableAllHooks").and_then(Value::as_bool) == Some(true) {
        r.bad("\"disableAllHooks\": true — Claude Code ignore tous les hooks");
    }

    let mut command = None;
    let mut found = Vec::new();
    for event in install::HOOK_EVENTS {
        let cmd = settings["hooks"][event]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|g| g["hooks"].as_array().into_iter().flatten())
            .filter_map(|h| h["command"].as_str())
            .find(|c| c.contains("bw-hook"));
        match cmd {
            Some(c) => {
                found.push(event);
                command.get_or_insert_with(|| c.to_owned());
            }
            None => r.bad(format!("pas de hook BoringWindows pour {event}")),
        }
    }
    if found.len() == install::HOOK_EVENTS.len() {
        r.ok(format!("{} événements branchés", found.len()));
    }
    if let Some(c) = &command {
        r.info(format!("commande : {c}"));
    } else {
        r.info("→ clic droit sur l'icône BoringWindows › « Claude Code : installer les hooks… »");
    }
    command
}

fn check_binary(r: &mut Report, installer: &Installer, app_exe: &Path) {
    let path = &installer.binary_path;
    match (path.metadata(), app_exe.metadata()) {
        (Ok(bin), Ok(app)) => {
            r.ok(format!("{} ({} Ko)", path.display(), bin.len() / 1024));
            if bin.len() != app.len() {
                r.info("copie différente de l'exécutable courant (mise à jour au prochain lancement de l'app)");
            }
        }
        (Err(_), _) => r.bad(format!("{} introuvable", path.display())),
        (_, Err(e)) => r.info(format!("exécutable courant illisible : {e}")),
    }
}

fn doctor_message(kind: &str, notification: Option<&str>) -> Message {
    Message {
        v: ipc::PROTOCOL_VERSION,
        wants_reply: false,
        ancestors: Vec::new(),
        console_window: None,
        event: HookEvent {
            session_id: SESSION.into(),
            cwd: "diagnostic".into(),
            kind: kind.into(),
            notification_type: notification.map(str::to_owned),
            ..HookEvent::default()
        },
    }
}

fn check_app(r: &mut Report, endpoint: &str) -> bool {
    r.info(format!("canal : {endpoint}"));
    let start = Instant::now();
    match hook::relay(endpoint, &doctor_message("SessionEnd", None)) {
        Outcome::Sent => {
            r.ok(format!("l'app répond ({} ms)", start.elapsed().as_millis()));
            true
        }
        other => {
            r.bad(format!("{other}"));
            r.info("→ lance BoringWindows (et le module claude : [modules.claude] enabled = true)");
            false
        }
    }
}

/// Exécute la commande du hook comme Claude Code : via bash (Git Bash sous
/// Windows), JSON sur stdin.
fn check_like_claude(r: &mut Report, command: &str) {
    let payload = serde_json::json!({
        "session_id": SESSION,
        "cwd": "diagnostic",
        "hook_event_name": "SessionEnd",
        "reason": "other",
    })
    .to_string();

    for shell in shells() {
        let start = Instant::now();
        let child = Command::new(shell.0)
            .args(shell.1)
            .arg(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(_) => {
                r.info(format!("{} : non disponible", shell.0));
                continue;
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(payload.as_bytes());
        }
        match wait_with_timeout(child, Duration::from_secs(10)) {
            Some(out) if out.status.success() => {
                r.ok(format!(
                    "via {} : code 0 en {} ms",
                    shell.0,
                    start.elapsed().as_millis()
                ));
            }
            Some(out) => {
                r.bad(format!("via {} : échec ({})", shell.0, out.status));
                let err = String::from_utf8_lossy(&out.stderr);
                for line in err.lines().take(5) {
                    r.info(line);
                }
            }
            None => r.bad(format!("via {} : bloqué plus de 10 s", shell.0)),
        }
    }
}

fn shells() -> Vec<(&'static str, &'static [&'static str])> {
    if cfg!(windows) {
        vec![("bash", &["-c"]), ("cmd", &["/C"])]
    } else {
        vec![("sh", &["-c"])]
    }
}

fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Option<std::process::Output> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                return None;
            }
        }
    }
}

fn check_journal(r: &mut Report) {
    let path = install::hook_log_path();
    r.info(format!("fichier : {}", path.display()));
    let Ok(text) = std::fs::read_to_string(&path) else {
        r.bad("aucun journal : Claude Code n'a jamais lancé le relais");
        r.info("→ relance tes sessions Claude Code après l'installation des hooks");
        return;
    };
    let lines: Vec<&str> = text.lines().collect();
    for line in &lines[lines.len().saturating_sub(12)..] {
        r.info(line);
    }
    if lines
        .iter()
        .rev()
        .take(12)
        .any(|l| l.contains("injoignable"))
    {
        r.bad("des appels récents n'ont pas joint l'app (voir ci-dessus)");
    }
}

fn visual_test(endpoint: &str) {
    let _ = hook::relay(endpoint, &doctor_message("SessionStart", None));
    let _ = hook::relay(
        endpoint,
        &doctor_message("Notification", Some("idle_prompt")),
    );
    std::thread::sleep(Duration::from_secs(6));
    let _ = hook::relay(endpoint, &doctor_message("SessionEnd", None));
}
