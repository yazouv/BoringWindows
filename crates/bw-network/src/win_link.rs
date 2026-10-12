//! État de la connexion via `Windows.Networking.Connectivity` : un événement
//! signale chaque changement de réseau, l'état est relu une fois la rafale passée.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;
use windows::Networking::Connectivity::{
    ConnectionProfile, NetworkConnectivityLevel, NetworkInformation,
    NetworkStatusChangedEventHandler,
};

use crate::{Link, Medium};

/// Signal envoyé au thread d'écoute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wake {
    /// Windows signale un changement : relire l'état.
    Changed,
    Stop,
}

/// Silence attendu avant de relire : un changement de réseau déclenche une
/// rafale d'événements, dont les premiers montrent des états transitoires.
const SETTLE: Duration = Duration::from_millis(800);

/// `IanaInterfaceType` : tunnel (VPN Windows, WireGuard…).
const IANA_TUNNEL: u32 = 131;
const IANA_ETHERNET: u32 = 6;
const IANA_WIFI: u32 = 71;

fn medium(profile: &ConnectionProfile) -> Medium {
    if profile.IsWlanConnectionProfile().unwrap_or(false) {
        return Medium::Wifi;
    }
    if profile.IsWwanConnectionProfile().unwrap_or(false) {
        return Medium::Cellular;
    }
    match profile
        .NetworkAdapter()
        .and_then(|a| a.IanaInterfaceType())
        .unwrap_or(0)
    {
        IANA_WIFI => Medium::Wifi,
        IANA_ETHERNET => Medium::Ethernet,
        _ => Medium::Other,
    }
}

/// Un profil connecté dont la carte est un tunnel.
fn vpn_active() -> bool {
    let Ok(profiles) = NetworkInformation::GetConnectionProfiles() else {
        return false;
    };
    profiles.into_iter().any(|p| {
        p.GetNetworkConnectivityLevel()
            .is_ok_and(|level| level != NetworkConnectivityLevel::None)
            && p.NetworkAdapter()
                .and_then(|a| a.IanaInterfaceType())
                .is_ok_and(|t| t == IANA_TUNNEL)
    })
}

fn read() -> Link {
    let vpn = vpn_active();
    // Sans profil Internet, `GetInternetConnectionProfile` échoue : hors ligne.
    let Ok(profile) = NetworkInformation::GetInternetConnectionProfile() else {
        return Link {
            vpn,
            ..Link::offline()
        };
    };
    let online = profile
        .GetNetworkConnectivityLevel()
        .is_ok_and(|level| level == NetworkConnectivityLevel::InternetAccess);
    let medium = medium(&profile);
    let (ssid, bars) = if medium == Medium::Wifi {
        let ssid = profile
            .WlanConnectionProfileDetails()
            .and_then(|d| d.GetConnectedSsid())
            .ok()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());
        let bars = profile
            .GetSignalBars()
            .and_then(|b| b.Value())
            .ok()
            .map(|b| b.min(5));
        (ssid, bars)
    } else {
        (None, None)
    };
    Link {
        online,
        medium,
        ssid,
        bars,
        vpn,
    }
}

/// Thread dédié : inscription à l'événement, puis relecture à chaque signal
/// jusqu'à `Wake::Stop` (envoyé à l'arrêt du module).
pub(crate) fn spawn(tx: UnboundedSender<Link>) -> Sender<Wake> {
    let (wake_tx, wake_rx) = channel();
    let handler_tx = wake_tx.clone();
    std::thread::spawn(move || {
        if let Err(e) = run(&tx, handler_tx, &wake_rx) {
            log::warn!("réseau : état de la connexion indisponible : {e}");
        }
    });
    wake_tx
}

fn run(
    tx: &UnboundedSender<Link>,
    wake_tx: Sender<Wake>,
    wake: &Receiver<Wake>,
) -> windows::core::Result<()> {
    let first = read();
    log::info!("réseau : {first:?}");
    let _ = tx.send(first.clone());

    let handler = NetworkStatusChangedEventHandler::new(move |_| {
        let _ = wake_tx.send(Wake::Changed);
        Ok(())
    });
    let token = NetworkInformation::NetworkStatusChanged(&handler)?;

    let mut last = first;
    while let Ok(signal) = wake.recv() {
        let mut stop = signal == Wake::Stop;
        // Attend la fin de la rafale.
        while !stop {
            match wake.recv_timeout(SETTLE) {
                Ok(Wake::Stop) | Err(RecvTimeoutError::Disconnected) => stop = true,
                Ok(Wake::Changed) => {}
                Err(RecvTimeoutError::Timeout) => break,
            }
        }
        if stop {
            break;
        }
        let now = read();
        if now != last {
            log::debug!("réseau : {now:?}");
            last = now.clone();
            if tx.send(now).is_err() {
                break;
            }
        }
    }

    let _ = NetworkInformation::RemoveNetworkStatusChanged(token);
    Ok(())
}
