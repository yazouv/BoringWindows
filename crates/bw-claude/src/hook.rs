//! Le relais exécuté par Claude Code (`boringwindows hook`).
//!
//! Règle d'or : ne jamais bloquer ni casser une session Claude. Toute erreur
//! (app fermée, lente, plantée, entrée illisible) se termine par une sortie
//! silencieuse avec le code 0 ; Claude continue alors comme sans hook.

use std::io::{BufRead, BufReader, Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::event::HookEvent;
use crate::ipc::{self, Decision, Message, Reply};

/// Délai pour joindre l'app et lui remettre l'événement.
pub const CONNECT_BUDGET: Duration = Duration::from_millis(300);
/// Attente maximale d'une décision (sous le timeout de 300 s posé dans settings.json).
pub const REPLY_CAP: Duration = Duration::from_secs(290);

enum Step {
    Sent,
    Reply(Decision),
}

/// Ce qu'il s'est passé, pour le journal et le diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// L'app n'a pas pu être jointe (fermée, autre utilisateur, pipe introuvable…).
    Unreachable(String),
    /// Événement remis, aucune réponse attendue.
    Sent,
    /// Réponse de l'île à une demande de permission.
    Decided(Decision),
    /// Demande remise, mais pas de réponse exploitable (délai, app fermée entre-temps).
    NoReply,
}

impl Outcome {
    pub fn decision(&self) -> Option<Decision> {
        match self {
            Self::Decided(d) => Some(*d),
            _ => None,
        }
    }
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(e) => write!(f, "app injoignable ({e})"),
            Self::Sent => f.write_str("remis"),
            Self::Decided(d) => write!(f, "réponse : {d:?}"),
            Self::NoReply => f.write_str("remis, sans réponse"),
        }
    }
}

/// Point d'entrée : lit stdin, relaie, écrit éventuellement la décision sur
/// stdout. Retourne le code de sortie (toujours 0).
pub fn run() -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        journal("?", "stdin illisible");
        return 0;
    }
    let Some(event) = HookEvent::from_hook_input(&input) else {
        journal("?", "événement illisible");
        return 0;
    };
    let kind = event.kind.clone();
    let message = Message {
        v: ipc::PROTOCOL_VERSION,
        // Les questions (AskUserQuestion, plan) partent tout de suite au
        // terminal : l'île les signale sans les retenir.
        wants_reply: kind == "PermissionRequest"
            && !event
                .tool_name
                .as_deref()
                .is_some_and(crate::event::is_interactive_tool),
        ancestors: process::ancestors(),
        console_window: process::console_window(),
        event,
    };

    let endpoint = ipc::endpoint();
    let doctor = message.event.session_id == crate::doctor::SESSION;
    let outcome = relay(&endpoint, &message);
    // Les tests du diagnostic ne polluent pas le journal qu'il relit.
    if !doctor {
        journal(&kind, &outcome.to_string());
    }
    if let Some(decision) = outcome.decision() {
        print_decision(decision);
    }
    0
}

/// Envoie `message` et attend la décision si besoin.
pub fn relay(endpoint: &str, message: &Message) -> Outcome {
    let (tx, rx) = mpsc::channel();
    let endpoint = endpoint.to_owned();
    let Ok(line) = serde_json::to_string(message) else {
        return Outcome::Unreachable("message non sérialisable".into());
    };
    let wants_reply = message.wants_reply;

    // Le travail bloquant se fait dans un thread : le thread principal garde
    // la maîtrise des délais et peut sortir même si le pipe est figé.
    std::thread::spawn(move || {
        let deadline = Instant::now() + CONNECT_BUDGET;
        let mut stream = match connect(&endpoint, deadline) {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = stream
            .write_all(format!("{line}\n").as_bytes())
            .and_then(|()| stream.flush())
        {
            let _ = tx.send(Err(format!("écriture : {e}")));
            return;
        }
        let _ = tx.send(Ok(Step::Sent));
        if !wants_reply {
            return;
        }
        let mut reply = String::new();
        let mut reader = BufReader::new(stream.take(ipc::MAX_LINE));
        if reader.read_line(&mut reply).is_ok()
            && let Ok(reply) = serde_json::from_str::<Reply>(&reply)
        {
            let _ = tx.send(Ok(Step::Reply(reply.decision)));
        }
    });

    match rx.recv_timeout(CONNECT_BUDGET) {
        Ok(Ok(Step::Sent)) => {}
        Ok(Err(e)) => return Outcome::Unreachable(e),
        _ => return Outcome::Unreachable("pas de réponse en 300 ms".into()),
    }
    if !wants_reply {
        return Outcome::Sent;
    }
    match rx.recv_timeout(REPLY_CAP) {
        Ok(Ok(Step::Reply(decision))) => Outcome::Decided(decision),
        _ => Outcome::NoReply,
    }
}

/// Journal du relais (`hook.log`), une ligne par appel, pour le diagnostic.
/// Plafonné : au-delà de 256 Ko, l'ancien journal est mis de côté.
fn journal(kind: &str, outcome: &str) {
    const MAX: u64 = 256 * 1024;
    let path = crate::install::hook_log_path();
    if path.metadata().is_ok_and(|m| m.len() > MAX) {
        let _ = std::fs::rename(&path, path.with_extension("log.old"));
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let line = format!(
        "{:02}:{:02}:{:02} UTC  {kind:<18} {outcome}\n",
        secs / 3600 % 24,
        secs / 60 % 60,
        secs % 60
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

fn print_decision(decision: Decision) {
    let behavior = match decision {
        Decision::Allow => "allow",
        Decision::Deny => "deny",
        Decision::Ask => return,
    };
    let mut out = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": { "behavior": behavior }
        }
    });
    if decision == Decision::Deny {
        out["hookSpecificOutput"]["decision"]["message"] =
            bw_i18n::tr!("Denied from BoringWindows", "Refusé depuis BoringWindows").into();
    }
    println!("{out}");
}

#[cfg(windows)]
fn connect(endpoint: &str, deadline: Instant) -> std::io::Result<std::fs::File> {
    // ERROR_PIPE_BUSY : toutes les instances sont occupées, on réessaie.
    const ERROR_PIPE_BUSY: i32 = 231;
    loop {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(endpoint)
        {
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            other => return other,
        }
    }
}

#[cfg(unix)]
fn connect(endpoint: &str, _deadline: Instant) -> std::io::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(endpoint)
}

#[cfg(windows)]
mod process {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, FreeConsole, GetConsoleWindow,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::GetCurrentProcessId;

    pub fn ancestors() -> Vec<u32> {
        let mut parents = std::collections::HashMap::new();
        // SAFETY: instantané Toolhelp parcouru puis fermé ; `entry` est
        // initialisé avec sa taille comme l'exige l'API.
        unsafe {
            let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
                return Vec::new();
            };
            let mut entry = PROCESSENTRY32W {
                dwSize: size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut ok = Process32FirstW(snap, &mut entry).is_ok();
            while ok {
                parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
                ok = Process32NextW(snap, &mut entry).is_ok();
            }
            let _ = CloseHandle(snap);
        }
        // SAFETY: appel sans argument.
        let mut pid = unsafe { GetCurrentProcessId() };
        let mut chain = Vec::new();
        while let Some(&parent) = parents.get(&pid) {
            if parent == 0 || chain.contains(&parent) || chain.len() >= 12 {
                break;
            }
            chain.push(parent);
            pid = parent;
        }
        chain
    }

    pub fn console_window() -> Option<i64> {
        // SAFETY: appels console sans pointeur ; la console empruntée au
        // parent est libérée aussitôt (les handles stdin/stdout ne changent pas).
        unsafe {
            let mut hwnd = GetConsoleWindow();
            if hwnd.is_invalid() && AttachConsole(ATTACH_PARENT_PROCESS).is_ok() {
                hwnd = GetConsoleWindow();
                let _ = FreeConsole();
            }
            (!hwnd.is_invalid()).then_some(hwnd.0 as i64)
        }
    }
}

#[cfg(unix)]
mod process {
    pub fn ancestors() -> Vec<u32> {
        let mut chain = vec![std::os::unix::process::parent_id()];
        // Linux : on remonte via /proc ; ailleurs, le parent direct suffit.
        while chain.len() < 12 {
            let pid = *chain.last().unwrap_or(&0);
            let Some(parent) = linux_parent(pid) else {
                break;
            };
            if parent <= 1 || chain.contains(&parent) {
                break;
            }
            chain.push(parent);
        }
        chain
    }

    fn linux_parent(pid: u32) -> Option<u32> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // Le nom du processus est entre parenthèses et peut contenir des espaces.
        stat.rsplit_once(')')?
            .1
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
    }

    pub fn console_window() -> Option<i64> {
        None
    }
}
