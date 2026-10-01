//! `boringwindows doctor` : vérifie toute la chaîne Claude Code → île et dit
//! précisément où elle casse.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use bw_i18n::tr;
use serde_json::Value;

use crate::event::HookEvent;
use crate::hook::{self, Outcome};
use crate::install::{self, Installer};
use crate::ipc::{self, Message};

/// Session fictive utilisée pour les tests (affichée « diagnostic »).
pub const SESSION: &str = "bw-doctor";

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

    fn section(&mut self, title: impl std::fmt::Display) {
        let _ = writeln!(self.text, "\n{title}");
    }
}

/// Lance le diagnostic et retourne le rapport. `app_exe` : l'exécutable courant.
pub fn run(app_exe: &Path) -> String {
    let mut r = Report {
        text: tr!(
            "BoringWindows × Claude Code diagnostic\n",
            "Diagnostic BoringWindows × Claude Code\n"
        ),
        problems: 0,
    };
    let installer = Installer::default();
    let endpoint = ipc::endpoint();

    r.section(tr!(
        "1. Hooks in settings.json",
        "1. Hooks dans settings.json"
    ));
    let command = check_settings(&mut r, &installer);

    r.section(tr!("2. Relay", "2. Relais"));
    check_binary(&mut r, &installer, app_exe);

    r.section(tr!(
        "3. BoringWindows running",
        "3. BoringWindows en cours d'exécution"
    ));
    let reachable = check_app(&mut r, &endpoint);

    // Lu avant le test 4, qui écrit lui-même dans le journal.
    let journal = std::fs::read_to_string(install::hook_log_path()).ok();

    r.section(tr!(
        "4. Relay launched the way Claude Code does",
        "4. Relais lancé comme le fait Claude Code"
    ));
    if let Some(command) = &command {
        check_like_claude(&mut r, command);
    } else {
        r.info(tr!(
            "(skipped: hooks not installed)",
            "(sauté : hooks non installés)"
        ));
    }

    r.section(tr!(
        "5. Relay log (latest calls from Claude Code)",
        "5. Journal du relais (derniers appels de Claude Code)"
    ));
    check_journal(&mut r, journal);

    if reachable {
        r.section(tr!("6. Visual test", "6. Test visuel"));
        visual_test(&endpoint);
        r.info(tr!(
            "The island should show \"diagnostic · waiting for you\" for 6 seconds.",
            "L'île doit afficher « diagnostic · attend ta réponse » pendant 6 secondes."
        ));
    }

    let _ = writeln!(
        r.text,
        "\n{}",
        if r.problems == 0 {
            tr!(
                "No problem found. If the island stays empty, restart your Claude Code sessions \
                 (hooks are read at startup), then check the log (point 5).",
                "Aucun problème détecté. Si l'île reste vide, relance tes sessions Claude Code \
                 (les hooks sont lus au démarrage) puis regarde le journal (point 5)."
            )
        } else {
            tr!(
                "{} problem(s) found, see the [!!] lines.",
                "{} problème(s) détecté(s), voir les lignes [!!].",
                r.problems
            )
        }
    );
    r.text
}

fn check_settings(r: &mut Report, installer: &Installer) -> Option<String> {
    let path = &installer.settings_path;
    r.info(tr!("file: {}", "fichier : {}", path.display()));
    let settings = match installer.read() {
        Ok(s) => s,
        Err(e) => {
            r.bad(tr!("cannot read: {e:#}", "lecture impossible : {e:#}"));
            return None;
        }
    };
    if !path.exists() {
        r.bad(tr!(
            "the file doesn't exist: hooks never installed",
            "le fichier n'existe pas : hooks jamais installés"
        ));
    }
    if settings.get("disableAllHooks").and_then(Value::as_bool) == Some(true) {
        r.bad(tr!(
            "\"disableAllHooks\": true — Claude Code ignores every hook",
            "\"disableAllHooks\": true — Claude Code ignore tous les hooks"
        ));
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
            None => r.bad(tr!(
                "no BoringWindows hook for {event}",
                "pas de hook BoringWindows pour {event}"
            )),
        }
    }
    if found.len() == install::HOOK_EVENTS.len() {
        r.ok(tr!(
            "{} events connected",
            "{} événements branchés",
            found.len()
        ));
        if !install::is_current(&settings, &install::hook_command(&installer.binary_path)) {
            r.info(tr!(
                "hooks from an older version: updated next time the app starts",
                "hooks d'une version précédente : mis à jour au prochain lancement de l'app"
            ));
        }
    }
    if let Some(c) = &command {
        r.info(tr!("command: {c}", "commande : {c}"));
    } else {
        r.info(tr!(
            "→ right-click the BoringWindows icon › \"Claude Code: install hooks…\"",
            "→ clic droit sur l'icône BoringWindows › « Claude Code : installer les hooks… »"
        ));
    }
    command
}

fn check_binary(r: &mut Report, installer: &Installer, app_exe: &Path) {
    let path = &installer.binary_path;
    match (path.metadata(), app_exe.metadata()) {
        (Ok(bin), Ok(app)) => {
            r.ok(tr!(
                "{} ({} KB)",
                "{} ({} Ko)",
                path.display(),
                bin.len() / 1024
            ));
            if bin.len() != app.len() {
                r.info(tr!("copy differs from the current executable (updated next time the app starts)", "copie différente de l'exécutable courant (mise à jour au prochain lancement de l'app)"));
            }
        }
        (Err(_), _) => r.bad(tr!("{} not found", "{} introuvable", path.display())),
        (_, Err(e)) => r.info(tr!(
            "current executable unreadable: {e}",
            "exécutable courant illisible : {e}"
        )),
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
    r.info(tr!("channel: {endpoint}", "canal : {endpoint}"));
    let start = Instant::now();
    match hook::relay(endpoint, &doctor_message("SessionEnd", None)) {
        Outcome::Sent => {
            r.ok(tr!(
                "the app responds ({} ms)",
                "l'app répond ({} ms)",
                start.elapsed().as_millis()
            ));
            true
        }
        other => {
            r.bad(format!("{other}"));
            r.info(tr!(
                "→ BoringWindows isn't running: start it (cargo run) in another terminal,",
                "→ BoringWindows n'est pas lancé : démarre-le (cargo run) dans un autre terminal,"
            ));
            r.info(tr!(
                "  leave it open, then run this diagnostic again.",
                "  laisse-le ouvert, puis relance ce diagnostic."
            ));
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
        let child = shell
            .command(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(_) => {
                r.info(tr!("{}: not available", "{} : non disponible", shell.name));
                continue;
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(payload.as_bytes());
        }
        match wait_with_timeout(child, Duration::from_secs(10)) {
            Some(out) if out.status.success() => {
                r.ok(tr!(
                    "via {}: exit code 0 in {} ms",
                    "via {} : code 0 en {} ms",
                    shell.name,
                    start.elapsed().as_millis()
                ));
            }
            Some(out) => {
                r.bad(tr!(
                    "via {}: failed ({})",
                    "via {} : échec ({})",
                    shell.name,
                    out.status
                ));
                let err = String::from_utf8_lossy(&out.stderr);
                for line in err.lines().take(5) {
                    r.info(line);
                }
            }
            None => r.bad(tr!(
                "via {}: stuck for more than 10 s",
                "via {} : bloqué plus de 10 s",
                shell.name
            )),
        }
    }
}

/// Interpréteur dans lequel Claude Code peut lancer un hook.
struct Shell {
    name: String,
    program: std::path::PathBuf,
    /// cmd.exe ne comprend pas l'échappement `\"` de Rust : ligne passée telle quelle.
    raw: bool,
}

impl Shell {
    fn command(&self, line: &str) -> Command {
        let mut cmd = Command::new(&self.program);
        if self.raw {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.raw_arg("/C").raw_arg(line);
            }
        } else {
            cmd.arg("-c").arg(line);
        }
        cmd
    }
}

#[cfg(windows)]
fn shells() -> Vec<Shell> {
    let mut shells = Vec::new();
    // Claude Code utilise Git Bash, jamais le bash de WSL (System32\bash.exe).
    match git_bash() {
        Some(program) => shells.push(Shell {
            name: format!("Git Bash ({})", program.display()),
            program,
            raw: false,
        }),
        None => shells.push(Shell {
            name: tr!("Git Bash (not found)", "Git Bash (introuvable)"),
            program: "git-bash-introuvable".into(),
            raw: false,
        }),
    }
    shells.push(Shell {
        name: "cmd".into(),
        program: "cmd".into(),
        raw: true,
    });
    shells
}

#[cfg(not(windows))]
fn shells() -> Vec<Shell> {
    vec![Shell {
        name: "sh".into(),
        program: "sh".into(),
        raw: false,
    }]
}

/// Même recherche que Claude Code : `CLAUDE_CODE_GIT_BASH_PATH`, sinon le
/// bash livré avec le `git` du PATH, sinon l'emplacement par défaut.
#[cfg(windows)]
fn git_bash() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    if let Some(p) = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH").map(PathBuf::from)
        && p.is_file()
    {
        return Some(p);
    }
    let from_path = std::env::var_os("PATH").into_iter().flat_map(|p| {
        std::env::split_paths(&p)
            .filter(|dir| dir.join("git.exe").is_file())
            .flat_map(|dir| {
                // …\Git\cmd\git.exe → …\Git\bin\bash.exe
                let root = dir.parent().map(PathBuf::from).unwrap_or_default();
                [root.join("bin").join("bash.exe"), dir.join("bash.exe")]
            })
            .collect::<Vec<_>>()
    });
    let defaults = [
        PathBuf::from(r"C:\Program Files\Git\bin\bash.exe"),
        PathBuf::from(r"C:\Program Files (x86)\Git\bin\bash.exe"),
    ];
    from_path.chain(defaults).find(|p| p.is_file())
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

fn check_journal(r: &mut Report, journal: Option<String>) {
    let path = install::hook_log_path();
    r.info(tr!("file: {}", "fichier : {}", path.display()));
    let Some(text) = journal else {
        r.bad(tr!(
            "no log: Claude Code never ran the relay",
            "aucun journal : Claude Code n'a jamais lancé le relais"
        ));
        r.info(tr!(
            "→ restart your Claude Code sessions after installing the hooks",
            "→ relance tes sessions Claude Code après l'installation des hooks"
        ));
        return;
    };
    let lines: Vec<&str> = text.lines().collect();
    for line in &lines[lines.len().saturating_sub(12)..] {
        r.info(line);
    }
    // Seul le dernier appel compte : les échecs plus anciens datent souvent
    // d'un moment où l'app était simplement fermée.
    if lines.last().is_some_and(|l| l.contains("injoignable")) {
        r.bad(tr!(
            "Claude's last call didn't reach the app (was it running?)",
            "le dernier appel de Claude n'a pas joint l'app (était-elle lancée ?)"
        ));
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
