//! État de la connexion via `Windows.Networking.Connectivity` : un événement
//! signale chaque changement de réseau, l'état est relu une fois la rafale passée.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;
use windows::Networking::Connectivity::{
    ConnectionProfile, NetworkConnectivityLevel, NetworkInformation,
    NetworkStatusChangedEventHandler,
};

use windows::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
    IP_ADAPTER_ADDRESSES_LH,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6,
};

use crate::{Link, Medium, looks_like_vpn, usable_address};

/// `ERROR_BUFFER_OVERFLOW` : le tampon est trop petit, `size` donne la taille voulue.
const ERROR_BUFFER_OVERFLOW_CODE: u32 = 111;

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

/// Un VPN est-il connecté ? Un adaptateur en service (`IfOperStatusUp`), avec une
/// adresse utilisable, qui a le type ou le nom d'un VPN (`looks_like_vpn`).
///
/// Le type « tunnel » de Windows ne suffit pas : OpenVPN (cartes TAP et DCO) se
/// déclare en `IfType` 6 ou 53.
fn vpn_active() -> bool {
    let mut size = 16 * 1024u32;
    let mut buffer: Vec<u64>;
    // SAFETY: tampon aligné (u64) de `size` octets que Windows remplit d'une
    // liste chaînée ; elle est lue sans sortir du tampon, qui vit jusqu'à la fin.
    unsafe {
        let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
        loop {
            buffer = vec![0u64; (size as usize).div_ceil(8)];
            let head = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
            match GetAdaptersAddresses(u32::from(AF_UNSPEC.0), flags, None, Some(head), &mut size) {
                0 => break,
                ERROR_BUFFER_OVERFLOW_CODE => {}
                _ => return false,
            }
        }
        let mut adapter = buffer.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        while !adapter.is_null() {
            let a = &*adapter;
            if a.OperStatus == IfOperStatusUp
                && !a.Description.is_null()
                && looks_like_vpn(a.IfType, &a.Description.to_string().unwrap_or_default())
                && has_usable_address(a)
            {
                return true;
            }
            adapter = a.Next;
        }
    }
    false
}

/// L'adaptateur a-t-il une adresse IP en service ?
///
/// # Safety
/// `adapter` doit venir d'un tampon de `GetAdaptersAddresses` encore valide.
unsafe fn has_usable_address(adapter: &IP_ADAPTER_ADDRESSES_LH) -> bool {
    let mut address = adapter.FirstUnicastAddress;
    // SAFETY: chaîne d'adresses du même tampon, terminée par un pointeur nul.
    unsafe {
        while !address.is_null() {
            let unicast = &*address;
            let socket = unicast.Address.lpSockaddr;
            if !socket.is_null() {
                let ip = match (*socket).sa_family {
                    AF_INET => Some(IpAddr::V4(Ipv4Addr::from(
                        (*socket.cast::<SOCKADDR_IN>())
                            .sin_addr
                            .S_un
                            .S_addr
                            .to_ne_bytes(),
                    ))),
                    AF_INET6 => Some(IpAddr::V6(Ipv6Addr::from(
                        (*socket.cast::<SOCKADDR_IN6>()).sin6_addr.u.Byte,
                    ))),
                    _ => None,
                };
                if ip.is_some_and(usable_address) {
                    return true;
                }
            }
            address = unicast.Next;
        }
    }
    false
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
