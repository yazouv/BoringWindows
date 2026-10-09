//! Lecture du centre de notifications de Windows (`UserNotificationListener`).
//!
//! Exception à la règle « pas de polling » : l'événement `NotificationChanged`
//! est refusé aux applications sans identité de package (MSIX), alors que la
//! lecture de la liste, elle, leur est permise. On relit donc la liste toutes
//! les [`POLL`] ; seuls les identifiants inconnus sont détaillés, le reste
//! vient d'un cache.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus as Access,
};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};

use crate::inbox::{AppIcon, Notification};

const POLL: Duration = Duration::from_secs(2);
/// Unités WinRT : 100 ns.
const TICKS_PER_SEC: i64 = 10_000_000;
/// Écart entre l'époque Windows (1601) et Unix (1970), en secondes.
const EPOCH_GAP_SECS: i64 = 11_644_473_600;

pub(crate) enum Command {
    /// Effacer ces notifications du centre de notifications.
    Remove(Vec<u32>),
    Stop,
}

pub(crate) fn spawn<F>(out: F) -> anyhow::Result<Sender<Command>>
where
    F: Fn(Vec<Notification>) + Send + 'static,
{
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("bw-notify".into())
        .spawn(move || {
            if let Err(e) = run(&rx, out) {
                log::error!("notifications : lecture impossible : {e}");
            }
        })?;
    Ok(tx)
}

fn run<F: Fn(Vec<Notification>)>(rx: &Receiver<Command>, out: F) -> windows::core::Result<()> {
    // COM pour les icônes (le shell) ; WinRT s'en accommode.
    // SAFETY: initialisation du thread courant, une seule fois.
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        );
    }
    let listener = UserNotificationListener::Current()?;
    let mut access = listener.GetAccessStatus()?;
    if access == Access::Unspecified {
        access = listener.RequestAccessAsync()?.join()?;
    }
    if access != Access::Allowed {
        log::warn!(
            "notifications : accès refusé par Windows (Paramètres › Confidentialité › Notifications)"
        );
        return Ok(());
    }
    log::info!("notifications : lecture du centre de notifications");

    let mut cache: HashMap<u32, Notification> = HashMap::new();
    // Icônes par application : lues une fois.
    let mut icons: HashMap<String, Option<AppIcon>> = HashMap::new();
    let mut last_ids: Option<Vec<u32>> = None;
    loop {
        match read(&listener, &mut cache, &mut icons) {
            Ok(list) => {
                let mut ids: Vec<u32> = list.iter().map(|n| n.id).collect();
                ids.sort_unstable();
                if last_ids.as_ref() != Some(&ids) {
                    last_ids = Some(ids);
                    out(list);
                }
            }
            Err(e) => log::debug!("notifications : lecture : {e}"),
        }
        match rx.recv_timeout(POLL) {
            Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => return Ok(()),
            Ok(Command::Remove(ids)) => {
                for id in ids {
                    if let Err(e) = listener.RemoveNotification(id) {
                        log::debug!("notifications : effacement de {id} : {e}");
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

fn read(
    listener: &UserNotificationListener,
    cache: &mut HashMap<u32, Notification>,
    icons: &mut HashMap<String, Option<AppIcon>>,
) -> windows::core::Result<Vec<Notification>> {
    let list = listener
        .GetNotificationsAsync(NotificationKinds::Toast)?
        .join()?;
    let mut current = Vec::new();
    for n in list {
        let Ok(id) = n.Id() else { continue };
        let notification = match cache.get(&id) {
            Some(known) => known.clone(),
            None => match details(&n, id) {
                Ok(mut details) => {
                    details.icon = icons
                        .entry(details.app_id.clone())
                        .or_insert_with(|| crate::icon::app_icon(&details.app_id))
                        .clone();
                    details
                }
                Err(e) => {
                    log::debug!("notifications : {id} illisible : {e}");
                    continue;
                }
            },
        };
        current.push(notification);
    }
    cache.clear();
    cache.extend(current.iter().map(|n| (n.id, n.clone())));
    Ok(current)
}

fn details(n: &UserNotification, id: u32) -> windows::core::Result<Notification> {
    let info = n.AppInfo()?;
    let app = info.DisplayInfo()?.DisplayName()?.to_string();
    let app_id = info.AppUserModelId()?.to_string();
    let texts: Vec<String> = n
        .Notification()?
        .Visual()?
        .GetBinding(&KnownNotificationBindings::ToastGeneric()?)?
        .GetTextElements()?
        .into_iter()
        .filter_map(|t| t.Text().ok())
        .map(|t| one_line(&t.to_string()))
        .filter(|t| !t.is_empty())
        .collect();
    let unix = n.CreationTime()?.UniversalTime / TICKS_PER_SEC - EPOCH_GAP_SECS;
    let at = UNIX_EPOCH + Duration::from_secs(unix.max(0) as u64);
    Ok(Notification {
        id,
        app,
        app_id,
        title: texts.first().cloned().unwrap_or_default(),
        body: texts
            .get(1..)
            .map(|rest| rest.join(" · "))
            .unwrap_or_default(),
        lines: texts,
        icon: None,
        at: at.min(SystemTime::now()),
    })
}

/// Espaces et retours à la ligne ramenés à un seul espace.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
