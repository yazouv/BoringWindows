//! Suivi des sessions Claude Code à partir des événements reçus. Logique pure :
//! le temps est passé en paramètre pour rester testable.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use bw_core::Attention;

use crate::event::{HookEvent, project_name};
use crate::ipc::Decision;

/// Une session sans nouvelles depuis ce délai est oubliée (Claude fermé brutalement).
const STALE_AFTER: Duration = Duration::from_secs(3 * 3600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Idle,
    Working,
    /// Une demande de permission attend une réponse dans l'île.
    Permission,
    /// Claude attend l'utilisateur dans le terminal.
    NeedsYou,
    Done,
}

#[derive(Debug, Clone)]
struct Session {
    project: String,
    kind: SessionKind,
    detail: Option<String>,
    done_until: Option<Instant>,
    last_seen: Instant,
    ancestors: Vec<u32>,
    console_window: Option<i64>,
}

#[derive(Debug, Clone)]
struct Prompt {
    id: u64,
    session_id: String,
    tool: String,
    detail: String,
    deadline: Instant,
}

/// Ce que l'UI affiche pour une session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionView {
    pub id: String,
    pub project: String,
    pub kind: SessionKind,
    pub status: String,
    pub ancestors: Vec<u32>,
    pub console_window: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptView {
    pub id: u64,
    pub session_id: String,
    pub project: String,
    pub tool: String,
    pub detail: String,
}

/// État complet publié vers l'UI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    pub sessions: Vec<SessionView>,
    /// Demande de permission la plus ancienne en attente.
    pub prompt: Option<PromptView>,
    pub attention: Attention,
    pub summary: Option<String>,
}

#[derive(Debug, Default)]
pub struct Tracker {
    sessions: BTreeMap<String, Session>,
    prompts: Vec<Prompt>,
    done_for: Duration,
}

impl Tracker {
    pub fn new(done_for: Duration) -> Self {
        Self {
            done_for,
            ..Self::default()
        }
    }

    pub fn on_event(
        &mut self,
        e: &HookEvent,
        ancestors: &[u32],
        console_window: Option<i64>,
        now: Instant,
    ) {
        if e.kind == "SessionEnd" {
            self.sessions.remove(&e.session_id);
            return;
        }
        let session = self
            .sessions
            .entry(e.session_id.clone())
            .or_insert_with(|| Session {
                project: e.project(),
                kind: SessionKind::Idle,
                detail: None,
                done_until: None,
                last_seen: now,
                ancestors: Vec::new(),
                console_window: None,
            });
        session.last_seen = now;
        if !e.cwd.is_empty() {
            session.project = project_name(&e.cwd);
        }
        if !ancestors.is_empty() {
            session.ancestors = ancestors.to_vec();
        }
        if console_window.is_some() {
            session.console_window = console_window;
        }

        let (kind, detail) = match (e.kind.as_str(), e.notification_type.as_deref()) {
            ("SessionStart", _) => (SessionKind::Idle, None),
            ("UserPromptSubmit", _) => (SessionKind::Working, None),
            ("PreToolUse", _) => (SessionKind::Working, e.tool_name.clone()),
            // Sans réponse possible dans l'île, Claude demandera dans le terminal.
            ("PermissionRequest", _) => (SessionKind::NeedsYou, Some(permission_label(e))),
            ("Notification", Some("permission_prompt")) => {
                (SessionKind::NeedsYou, Some("demande une permission".into()))
            }
            ("Notification", Some("idle_prompt")) => {
                (SessionKind::NeedsYou, Some("attend ta réponse".into()))
            }
            ("Notification", Some("agent_needs_input" | "elicitation_dialog")) => {
                (SessionKind::NeedsYou, Some("a une question".into()))
            }
            ("Stop", _) => (SessionKind::Done, None),
            _ => return,
        };
        // Une demande en cours dans l'île garde la priorité sur un PreToolUse
        // concurrent (sous-agent) de la même session.
        if session.kind == SessionKind::Permission && kind == SessionKind::Working {
            return;
        }
        session.kind = kind;
        session.detail = detail;
        session.done_until = (kind == SessionKind::Done).then(|| now + self.done_for);
    }

    /// Enregistre une demande de permission à laquelle l'île peut répondre.
    pub fn add_prompt(&mut self, id: u64, e: &HookEvent, deadline: Instant) {
        if let Some(s) = self.sessions.get_mut(&e.session_id) {
            s.kind = SessionKind::Permission;
            s.detail = Some(permission_label(e));
        }
        self.prompts.push(Prompt {
            id,
            session_id: e.session_id.clone(),
            tool: e.tool_name.clone().unwrap_or_else(|| "outil".into()),
            detail: e.tool_detail.clone().unwrap_or_default(),
            deadline,
        });
    }

    /// Une demande est close (réponse, délai écoulé ou relais parti).
    pub fn resolve_prompt(&mut self, id: u64, decision: Decision) -> bool {
        let Some(pos) = self.prompts.iter().position(|p| p.id == id) else {
            return false;
        };
        let prompt = self.prompts.remove(pos);
        let still_pending = self
            .prompts
            .iter()
            .any(|p| p.session_id == prompt.session_id);
        if let Some(s) = self.sessions.get_mut(&prompt.session_id)
            && !still_pending
        {
            (s.kind, s.detail) = match decision {
                Decision::Allow | Decision::Deny => (SessionKind::Working, None),
                // Claude va poser la question dans le terminal.
                Decision::Ask => (SessionKind::NeedsYou, Some("demande une permission".into())),
            };
        }
        true
    }

    /// Fait avancer le temps. Retourne les demandes arrivées à échéance (à
    /// renvoyer au terminal) et indique si l'affichage a changé.
    pub fn tick(&mut self, now: Instant) -> (Vec<u64>, bool) {
        let mut changed = false;
        for s in self.sessions.values_mut() {
            if s.done_until.is_some_and(|t| t <= now) {
                s.kind = SessionKind::Idle;
                s.done_until = None;
                changed = true;
            }
        }
        let prompts = &self.prompts;
        let before = self.sessions.len();
        self.sessions.retain(|id, s| {
            now.duration_since(s.last_seen) < STALE_AFTER
                || prompts.iter().any(|p| &p.session_id == id)
        });
        changed |= self.sessions.len() != before;

        let expired: Vec<u64> = self
            .prompts
            .iter()
            .filter(|p| p.deadline <= now)
            .map(|p| p.id)
            .collect();
        for &id in &expired {
            changed |= self.resolve_prompt(id, Decision::Ask);
        }
        (expired, changed)
    }

    /// Prochain instant où `tick` aura quelque chose à faire.
    pub fn next_deadline(&self) -> Option<Instant> {
        let done = self.sessions.values().filter_map(|s| s.done_until);
        let prompts = self.prompts.iter().map(|p| p.deadline);
        let stale = self.sessions.values().map(|s| s.last_seen + STALE_AFTER);
        done.chain(prompts).chain(stale).min()
    }

    pub fn snapshot(&self) -> Snapshot {
        let sessions: Vec<SessionView> = self
            .sessions
            .iter()
            .map(|(id, s)| SessionView {
                id: id.clone(),
                project: s.project.clone(),
                kind: s.kind,
                status: status_text(s),
                ancestors: s.ancestors.clone(),
                console_window: s.console_window,
            })
            .collect();

        let prompt = self.prompts.first().map(|p| PromptView {
            id: p.id,
            session_id: p.session_id.clone(),
            project: self
                .sessions
                .get(&p.session_id)
                .map_or_else(|| "Claude".into(), |s| s.project.clone()),
            tool: p.tool.clone(),
            detail: p.detail.clone(),
        });

        let (attention, summary) = attention(&sessions);
        Snapshot {
            sessions,
            prompt,
            attention,
            summary,
        }
    }
}

fn permission_label(e: &HookEvent) -> String {
    format!(
        "autoriser {} ?",
        e.tool_name.as_deref().unwrap_or("un outil")
    )
}

fn status_text(s: &Session) -> String {
    match (s.kind, &s.detail) {
        (SessionKind::Idle, _) => "en pause".into(),
        (SessionKind::Working, Some(tool)) => tool.clone(),
        (SessionKind::Working, None) => "réfléchit…".into(),
        (SessionKind::Done, _) => "terminé".into(),
        (_, Some(d)) => d.clone(),
        (_, None) => "attend ta réponse".into(),
    }
}

/// Résumé de la pilule compacte : la session la plus urgente, et combien
/// d'autres sont actives.
fn attention(sessions: &[SessionView]) -> (Attention, Option<String>) {
    let level = |k: SessionKind| match k {
        SessionKind::Permission | SessionKind::NeedsYou => Attention::Urgent,
        SessionKind::Done => Attention::High,
        SessionKind::Working => Attention::Low,
        SessionKind::Idle => Attention::None,
    };
    // À niveau égal, une demande de permission (répondable dans l'île) passe devant.
    let Some(top) = sessions
        .iter()
        .max_by_key(|s| (level(s.kind), s.kind == SessionKind::Permission))
    else {
        return (Attention::None, None);
    };
    let top_level = level(top.kind);
    if top_level == Attention::None {
        return (Attention::None, None);
    }
    let others = sessions
        .iter()
        .filter(|s| s.id != top.id && level(s.kind) > Attention::None)
        .count();
    let mut summary = format!("{} · {}", top.project, top.status);
    if others > 0 {
        summary.push_str(&format!("  (+{others})"));
    }
    (top_level, Some(summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(session: &str, kind: &str) -> HookEvent {
        HookEvent {
            session_id: session.into(),
            cwd: format!("/dev/{session}"),
            kind: kind.into(),
            ..HookEvent::default()
        }
    }

    fn tool(session: &str, kind: &str, name: &str, detail: &str) -> HookEvent {
        HookEvent {
            tool_name: Some(name.into()),
            tool_detail: Some(detail.into()),
            ..ev(session, kind)
        }
    }

    fn notif(session: &str, ty: &str) -> HookEvent {
        HookEvent {
            notification_type: Some(ty.into()),
            ..ev(session, "Notification")
        }
    }

    fn tracker() -> (Tracker, Instant) {
        (Tracker::new(Duration::from_secs(8)), Instant::now())
    }

    #[test]
    fn lifecycle_working_done_idle_end() {
        let (mut t, now) = tracker();
        t.on_event(&ev("a", "SessionStart"), &[], None, now);
        assert_eq!(t.snapshot().attention, Attention::None);

        t.on_event(&ev("a", "UserPromptSubmit"), &[], None, now);
        t.on_event(&tool("a", "PreToolUse", "Bash", "ls"), &[], None, now);
        let s = t.snapshot();
        assert_eq!(s.attention, Attention::Low);
        assert_eq!(s.summary.as_deref(), Some("a · Bash"));

        t.on_event(&ev("a", "Stop"), &[], None, now);
        assert_eq!(t.snapshot().attention, Attention::High);
        assert_eq!(t.next_deadline(), Some(now + Duration::from_secs(8)));

        let (expired, changed) = t.tick(now + Duration::from_secs(9));
        assert!(expired.is_empty() && changed);
        assert_eq!(t.snapshot().sessions[0].kind, SessionKind::Idle);

        t.on_event(&ev("a", "SessionEnd"), &[], None, now);
        assert!(t.snapshot().sessions.is_empty());
    }

    #[test]
    fn notifications_need_you() {
        let (mut t, now) = tracker();
        t.on_event(&notif("a", "idle_prompt"), &[], None, now);
        let s = t.snapshot();
        assert_eq!(s.attention, Attention::Urgent);
        assert_eq!(s.summary.as_deref(), Some("a · attend ta réponse"));
        // Types sans intérêt ignorés.
        t.on_event(&notif("a", "auth_success"), &[], None, now);
        assert_eq!(t.snapshot().attention, Attention::Urgent);
    }

    #[test]
    fn prompt_allow_resumes_work() {
        let (mut t, now) = tracker();
        let req = tool("a", "PermissionRequest", "Bash", "rm -rf target");
        t.on_event(&req, &[], None, now);
        t.add_prompt(1, &req, now + Duration::from_secs(60));

        let s = t.snapshot();
        assert_eq!(s.attention, Attention::Urgent);
        assert_eq!(
            s.prompt,
            Some(PromptView {
                id: 1,
                session_id: "a".into(),
                project: "a".into(),
                tool: "Bash".into(),
                detail: "rm -rf target".into(),
            })
        );
        assert_eq!(s.summary.as_deref(), Some("a · autoriser Bash ?"));

        // Un PreToolUse concurrent n'efface pas la demande.
        t.on_event(&tool("a", "PreToolUse", "Read", "x"), &[], None, now);
        assert_eq!(t.snapshot().sessions[0].kind, SessionKind::Permission);

        assert!(t.resolve_prompt(1, Decision::Allow));
        assert!(!t.resolve_prompt(1, Decision::Allow));
        let s = t.snapshot();
        assert_eq!(s.prompt, None);
        assert_eq!(s.sessions[0].kind, SessionKind::Working);
    }

    #[test]
    fn prompt_timeout_falls_back_to_terminal() {
        let (mut t, now) = tracker();
        let req = tool("a", "PermissionRequest", "Edit", "main.rs");
        t.on_event(&req, &[], None, now);
        t.add_prompt(7, &req, now + Duration::from_secs(60));
        assert_eq!(t.next_deadline(), Some(now + Duration::from_secs(60)));

        let (expired, changed) = t.tick(now + Duration::from_secs(61));
        assert_eq!(expired, vec![7]);
        assert!(changed);
        let s = t.snapshot();
        assert_eq!(s.prompt, None);
        assert_eq!(s.sessions[0].kind, SessionKind::NeedsYou);
    }

    #[test]
    fn most_urgent_session_wins_with_count() {
        let (mut t, now) = tracker();
        t.on_event(&ev("a", "UserPromptSubmit"), &[], None, now);
        t.on_event(&notif("b", "permission_prompt"), &[], None, now);
        t.on_event(&ev("c", "SessionStart"), &[], None, now);
        let s = t.snapshot();
        assert_eq!(s.attention, Attention::Urgent);
        assert_eq!(
            s.summary.as_deref(),
            Some("b · demande une permission  (+1)")
        );
    }

    #[test]
    fn permission_beats_other_urgent_sessions() {
        let (mut t, now) = tracker();
        let req = tool("a", "PermissionRequest", "Bash", "ls");
        t.on_event(&req, &[], None, now);
        t.add_prompt(1, &req, now + Duration::from_secs(60));
        t.on_event(&notif("z", "idle_prompt"), &[], None, now);
        assert_eq!(
            t.snapshot().summary.as_deref(),
            Some("a · autoriser Bash ?  (+1)")
        );
    }

    #[test]
    fn stale_sessions_are_forgotten() {
        let (mut t, now) = tracker();
        t.on_event(&ev("a", "UserPromptSubmit"), &[42], Some(7), now);
        assert_eq!(t.snapshot().sessions[0].ancestors, vec![42]);
        let (_, changed) = t.tick(now + STALE_AFTER + Duration::from_secs(1));
        assert!(changed);
        assert!(t.snapshot().sessions.is_empty());
    }
}
