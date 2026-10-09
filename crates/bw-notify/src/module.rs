//! Module « notifications » : annonce dans la pilule chaque nouvelle
//! notification d'application, et garde les dernières pour l'onglet de l'île.

use bw_core::{Module, ModuleCtx};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::config::NotifyConfig;

pub const MODULE_ID: &str = "notifications";

/// Actions de l'UI, transmises à la tâche du module.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
enum UiAction {
    /// L'onglet a été consulté.
    Seen,
    /// Ne plus annoncer la notification en cours.
    Hide,
    Remove(u32),
    Clear,
    /// Ne pas déranger : les notifications arrivent sans être annoncées.
    DoNotDisturb(bool),
}

impl UiAction {
    /// `seen`, `hide`, `remove:<id>`, `clear`, `dnd:on`, `dnd:off`.
    fn parse(action: &str) -> Option<Self> {
        Some(match action {
            "seen" => Self::Seen,
            "hide" => Self::Hide,
            "clear" => Self::Clear,
            "dnd:on" => Self::DoNotDisturb(true),
            "dnd:off" => Self::DoNotDisturb(false),
            _ => Self::Remove(action.strip_prefix("remove:")?.parse().ok()?),
        })
    }
}

pub struct NotifyModule {
    config: NotifyConfig,
    actions: Option<UnboundedSender<UiAction>>,
    #[cfg(windows)]
    listener: Option<std::sync::mpsc::Sender<crate::listener::Command>>,
}

impl NotifyModule {
    pub fn new(config: NotifyConfig) -> Self {
        Self {
            config,
            actions: None,
            #[cfg(windows)]
            listener: None,
        }
    }

    pub const fn is_supported() -> bool {
        cfg!(windows)
    }
}

impl Module for NotifyModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    #[cfg(windows)]
    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        use bw_core::Attention;
        use tokio::time::Instant;

        use crate::inbox::{Inbox, Notification, summary};
        use crate::listener::{self, Command};

        let (list_tx, mut lists) = unbounded_channel::<Vec<Notification>>();
        let commands = listener::spawn(move |list| {
            let _ = list_tx.send(list);
        })?;
        self.listener = Some(commands.clone());
        let (action_tx, mut actions) = unbounded_channel::<UiAction>();
        self.actions = Some(action_tx);

        let config = self.config.clone();
        ctx.clone().spawn(async move {
            let mut inbox = Inbox::default();
            let mut announcing: Option<Notification> = None;
            let mut hide_at: Option<Instant> = None;
            let mut quiet = false;
            loop {
                let hide = async {
                    match hide_at {
                        Some(at) => tokio::time::sleep_until(at).await,
                        None => std::future::pending().await,
                    }
                };
                tokio::select! {
                    list = lists.recv() => {
                        let Some(list) = list else { break };
                        let list = list.into_iter().map(|n| redact(n, &config)).collect();
                        let latest = inbox.sync(list, &config).pop();
                        if let Some(latest) = latest.filter(|_| !quiet) {
                            log::info!("notifications : nouvelle notification de {}", latest.app);
                            ctx.set_attention(Attention::High, Some(summary(&latest)));
                            announcing = Some(latest);
                            hide_at = Some(Instant::now() + config.show_for());
                        }
                    }
                    action = actions.recv() => {
                        let Some(action) = action else { break };
                        match action {
                            UiAction::Seen => inbox.mark_seen(),
                            UiAction::Hide => hide_at = Some(Instant::now()),
                            UiAction::DoNotDisturb(on) => {
                                quiet = on;
                                if on && announcing.is_some() {
                                    hide_at = Some(Instant::now());
                                }
                            }
                            UiAction::Remove(id) => {
                                inbox.remove(id);
                                let _ = commands.send(Command::Remove(vec![id]));
                            }
                            UiAction::Clear => {
                                let ids = inbox.snapshot(None).recent.iter().map(|n| n.id).collect();
                                inbox.clear();
                                let _ = commands.send(Command::Remove(ids));
                            }
                        }
                    }
                    () = hide => {
                        ctx.set_attention(Attention::None, None);
                        announcing = None;
                        hide_at = None;
                    }
                }
                ctx.set_state(inbox.snapshot(announcing.clone()));
            }
        });
        Ok(())
    }

    #[cfg(not(windows))]
    fn start(&mut self, _ctx: ModuleCtx) -> anyhow::Result<()> {
        let _ = (&self.config, unbounded_channel::<UiAction>);
        log::info!("notifications : pas de centre de notifications sur ce système");
        Ok(())
    }

    fn on_action(&mut self, action: &str) {
        match (UiAction::parse(action), &self.actions) {
            (Some(a), Some(tx)) => {
                let _ = tx.send(a);
            }
            (None, _) => log::warn!("notifications : action inconnue {action:?}"),
            _ => {}
        }
    }
}

impl Drop for NotifyModule {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(tx) = &self.listener {
            let _ = tx.send(crate::listener::Command::Stop);
        }
    }
}

/// Sans `show_content`, seul le nom de l'application reste.
#[cfg_attr(not(windows), allow(dead_code))]
fn redact(mut n: crate::inbox::Notification, config: &NotifyConfig) -> crate::inbox::Notification {
    if !config.show_content {
        n.title = String::new();
        n.body = bw_i18n::tr!("New notification", "Nouvelle notification");
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_actions() {
        assert_eq!(UiAction::parse("seen"), Some(UiAction::Seen));
        assert_eq!(UiAction::parse("remove:42"), Some(UiAction::Remove(42)));
        assert_eq!(UiAction::parse("remove:x"), None);
        assert_eq!(
            UiAction::parse("dnd:on"),
            Some(UiAction::DoNotDisturb(true))
        );
        assert_eq!(
            UiAction::parse("dnd:off"),
            Some(UiAction::DoNotDisturb(false))
        );
        assert_eq!(UiAction::parse("boom"), None);
    }

    #[test]
    fn redaction_keeps_only_the_app() {
        bw_i18n::set(bw_i18n::Lang::Fr);
        let n = crate::inbox::Notification {
            id: 1,
            app: "Discord".into(),
            app_id: String::new(),
            title: "Arkyan".into(),
            body: "salut".into(),
            lines: vec!["Arkyan".into(), "salut".into()],
            icon: None,
            at: std::time::SystemTime::UNIX_EPOCH,
        };
        let hidden = redact(
            n.clone(),
            &NotifyConfig {
                show_content: false,
                ..NotifyConfig::default()
            },
        );
        assert_eq!(hidden.title, "");
        assert_eq!(hidden.body, "Nouvelle notification");
        assert_eq!(redact(n.clone(), &NotifyConfig::default()), n);
    }
}
