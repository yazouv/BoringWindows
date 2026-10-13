//! Réseau : le débit (affichable jusque dans la pilule repliée), le type de
//! connexion (Wi-Fi, filaire), le VPN, et l'annonce des changements.
//!
//! L'état de la connexion vient d'événements de Windows (aucune
//! interrogation). Le débit se mesure par échantillons, mais seulement quand
//! il est visible : île ouverte, ou pilule repliée réglée pour l'afficher.
//!
//! La logique (`format_rate`, `aggregate`, `link_notice`, `Ticker`) est pure :
//! elle reçoit les valeurs et dit quoi afficher.

mod config;
mod module;
#[cfg(windows)]
mod win_link;

use bw_core::Attention;
use bw_i18n::{Lang, tr};

pub use config::{CompactSpeed, NetworkConfig, Unit};
pub use module::{MODULE_ID, NetworkModule};

/// Type de la connexion à Internet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Medium {
    Wifi,
    Ethernet,
    Cellular,
    Other,
}

/// État de la connexion, tel que Windows le décrit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Accès à Internet.
    pub online: bool,
    pub medium: Medium,
    /// Nom du réseau Wi-Fi.
    pub ssid: Option<String>,
    /// Force du signal Wi-Fi (0 à 5 barres).
    pub bars: Option<u8>,
    /// Un tunnel VPN est connecté.
    pub vpn: bool,
}

impl Link {
    pub fn offline() -> Self {
        Self {
            online: false,
            medium: Medium::Other,
            ssid: None,
            bars: None,
            vpn: false,
        }
    }

    /// « Home Wi-Fi », « Ethernet » : texte court de l'en-tête.
    pub fn label(&self) -> String {
        if !self.online {
            return tr!("Offline", "Hors ligne");
        }
        match (self.medium, &self.ssid) {
            (Medium::Wifi, Some(ssid)) => ssid.clone(),
            (Medium::Wifi, None) => "Wi-Fi".to_owned(),
            (Medium::Ethernet, _) => tr!("Ethernet", "Ethernet"),
            (Medium::Cellular, _) => tr!("Cellular", "Données mobiles"),
            (Medium::Other, _) => tr!("Connected", "Connecté"),
        }
    }
}

/// Ce que publie le module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSnapshot {
    /// Débit descendant et montant (octets par seconde).
    pub down: u64,
    pub up: u64,
    /// `None` hors Windows : seul le débit est connu.
    pub link: Option<Link>,
    pub unit: Unit,
}

impl NetworkSnapshot {
    pub fn down_text(&self) -> String {
        format_rate(self.down, self.unit)
    }

    pub fn up_text(&self) -> String {
        format_rate(self.up, self.unit)
    }

    /// Icône de l'en-tête : `wifi`, `wifi-off`, `ethernet`, `shield` (VPN) ou `globe`.
    pub fn icon(&self) -> &'static str {
        match &self.link {
            None => "globe",
            Some(link) if !link.online => "wifi-off",
            Some(link) if link.vpn => "shield",
            Some(link) => match link.medium {
                Medium::Wifi => "wifi",
                Medium::Ethernet => "ethernet",
                Medium::Cellular | Medium::Other => "globe",
            },
        }
    }

    /// Hors ligne : l'icône prend la couleur d'alerte.
    pub fn offline(&self) -> bool {
        self.link.as_ref().is_some_and(|l| !l.online)
    }

    /// « Home · VPN · ↓ 2,3 Mo/s  ↑ 120 Ko/s » : détail montré au survol, dans
    /// la bulle sous l'en-tête.
    pub fn detail(&self) -> String {
        let mut parts = Vec::new();
        if let Some(link) = &self.link {
            parts.push(link.label());
            if link.vpn {
                parts.push("VPN".to_owned());
            }
        }
        if self.link.as_ref().is_none_or(|l| l.online) {
            parts.push(format!("↓ {}  ↑ {}", self.down_text(), self.up_text()));
        }
        parts.join(" · ")
    }
}

// --- Débit -------------------------------------------------------------------

/// « 2,3 Mo/s » (ou « 18 Mb/s » en bits) : une décimale sous 10, aucune au-delà.
pub fn format_rate(bytes_per_sec: u64, unit: Unit) -> String {
    let (value, steps) = match unit {
        Unit::Bytes => (bytes_per_sec as f64, ["B", "KB", "MB", "GB"]),
        Unit::Bits => (bytes_per_sec as f64 * 8.0, ["b", "kb", "Mb", "Gb"]),
    };
    // Unités décimales, comme les fournisseurs d'accès et les navigateurs.
    let mut value = value;
    let mut step = 0;
    while value >= 999.5 && step + 1 < steps.len() {
        value /= 1000.0;
        step += 1;
    }
    let number = if step == 0 || value >= 9.95 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    };
    let number = match bw_i18n::lang() {
        Lang::Fr => number.replace('.', ","),
        Lang::En => number,
    };
    let name = steps[step];
    // En français, l'octet s'écrit « o » : « Ko/s », « Mo/s ».
    let name = match (unit, bw_i18n::lang()) {
        (Unit::Bytes, Lang::Fr) => name.replace('B', "o"),
        _ => name.to_owned(),
    };
    format!("{number} {name}/s")
}

/// « ↓ 2,3 Mo/s  ↑ 120 Ko/s » : texte de la pilule repliée.
pub fn compact_text(down: u64, up: u64, unit: Unit) -> String {
    format!(
        "↓ {}   ↑ {}",
        format_rate(down, unit),
        format_rate(up, unit)
    )
}

/// Total descendant et montant d'un relevé `(interface, reçus, envoyés)`.
///
/// Les octets d'un VPN passent deux fois sur l'ordinateur (dans le tunnel, puis
/// chiffrés sur la carte réelle) : avec un VPN, on garde l'interface la plus
/// chargée plutôt que la somme. La boucle locale est ignorée.
pub fn aggregate(samples: &[(&str, u64, u64)], vpn: bool) -> (u64, u64) {
    let real = samples
        .iter()
        .filter(|(name, ..)| !name.to_ascii_lowercase().contains("loopback"));
    if vpn {
        real.fold((0, 0), |(d, u), &(_, rx, tx)| (d.max(rx), u.max(tx)))
    } else {
        real.fold((0, 0), |(d, u), &(_, rx, tx)| (d + rx, u + tx))
    }
}

/// Décide quand le débit s'affiche dans la pilule repliée.
///
/// En mode `Active`, il apparaît dès que le seuil est atteint et ne disparaît
/// qu'après `HOLD` relevés calmes : pas de clignotement sur un trafic en dents
/// de scie.
#[derive(Debug, Clone)]
pub struct Ticker {
    mode: CompactSpeed,
    threshold: u64,
    calm: u32,
    shown: bool,
}

impl Ticker {
    /// Relevés calmes avant de masquer le débit.
    const HOLD: u32 = 3;

    pub fn new(mode: CompactSpeed, threshold: u64) -> Self {
        Self {
            mode,
            threshold,
            calm: 0,
            shown: false,
        }
    }

    /// Prend un relevé ; `true` si le débit doit s'afficher dans la pilule.
    pub fn feed(&mut self, down: u64, up: u64) -> bool {
        self.shown = match self.mode {
            CompactSpeed::Off => false,
            CompactSpeed::Always => true,
            CompactSpeed::Active => {
                if down.max(up) >= self.threshold {
                    self.calm = 0;
                    true
                } else {
                    self.calm = self.calm.saturating_add(1);
                    self.shown && self.calm < Self::HOLD
                }
            }
        };
        self.shown
    }
}

// --- VPN ---------------------------------------------------------------------

/// `IfType` des interfaces de type « tunnel » (VPN de Windows, WireGuard…).
const IF_TYPE_TUNNEL: u32 = 131;
/// Fragments du nom d'un adaptateur qui trahissent un VPN : OpenVPN (TAP, Wintun,
/// DCO), WireGuard, FortiClient, Cisco, Palo Alto…
const VPN_NAMES: [&str; 10] = [
    "openvpn",
    "tap-windows",
    "wintun",
    "wireguard",
    "nordlynx",
    "vpn",
    "anyconnect",
    "globalprotect",
    "pangp",
    "mullvad",
];
/// Adaptateurs de tunnel qui n'ont rien d'un VPN, ou virtuels (machines, conteneurs).
const NOT_VPN_NAMES: [&str; 9] = [
    "teredo",
    "6to4",
    "isatap",
    "ip-https",
    "hyper-v",
    "virtualbox",
    "vmware",
    "wsl",
    "docker",
];

/// L'adaptateur est-il celui d'un VPN ? `if_type` est l'`IfType` de Windows,
/// `description` le nom de la carte (« OpenVPN Data Channel Offload »).
pub fn looks_like_vpn(if_type: u32, description: &str) -> bool {
    let name = description.to_lowercase();
    if NOT_VPN_NAMES.iter().any(|n| name.contains(n)) {
        return false;
    }
    if_type == IF_TYPE_TUNNEL || VPN_NAMES.iter().any(|n| name.contains(n))
}

/// Une adresse qui prouve que l'adaptateur est réellement en service : ni
/// locale au lien (169.254.x.x, fe80::), ni de bouclage, ni non spécifiée. Un
/// adaptateur de VPN déconnecté n'a que des adresses locales au lien.
pub fn usable_address(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            !(v4.is_link_local() || v4.is_loopback() || v4.is_unspecified())
        }
        std::net::IpAddr::V6(v6) => {
            // fe80::/10
            let link_local = (v6.segments()[0] & 0xffc0) == 0xfe80;
            !(link_local || v6.is_loopback() || v6.is_unspecified())
        }
    }
}

// --- Connexion ---------------------------------------------------------------

/// Annonce à faire dans la pilule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub attention: Attention,
}

/// Compare deux états de la connexion et dit quoi annoncer, selon la config.
pub fn link_notice(prev: &Link, now: &Link, config: &NetworkConfig) -> Option<Notice> {
    let notice = |text: String, attention| Some(Notice { text, attention });

    if config.announce_connection && prev.online != now.online {
        return if now.online {
            notice(tr!("Back online", "Connexion rétablie"), Attention::High)
        } else {
            notice(
                tr!("No internet connection", "Plus de connexion Internet"),
                Attention::Urgent,
            )
        };
    }
    if config.announce_vpn && prev.vpn != now.vpn {
        return if now.vpn {
            notice(tr!("VPN connected", "VPN connecté"), Attention::High)
        } else {
            // Un VPN qui tombe expose le trafic : ça mérite plus qu'un coup d'œil.
            notice(tr!("VPN disconnected", "VPN déconnecté"), Attention::Urgent)
        };
    }
    if config.announce_wifi
        && now.online
        && now.medium == Medium::Wifi
        && now.ssid.is_some()
        && prev.ssid != now.ssid
    {
        let ssid = now.ssid.clone().unwrap_or_default();
        return notice(
            tr!("Connected to {ssid}", "Connecté à {ssid}"),
            Attention::High,
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    /// La langue est globale au processus : les tests qui en dépendent se
    /// passent le relais au lieu de se la disputer.
    static LANG: Mutex<()> = Mutex::new(());

    fn lang(lang: Lang) -> MutexGuard<'static, ()> {
        let guard = LANG.lock().unwrap_or_else(|e| e.into_inner());
        bw_i18n::set(lang);
        guard
    }

    fn wifi(ssid: &str, vpn: bool) -> Link {
        Link {
            online: true,
            medium: Medium::Wifi,
            ssid: Some(ssid.to_owned()),
            bars: Some(4),
            vpn,
        }
    }

    fn config() -> NetworkConfig {
        NetworkConfig {
            announce_wifi: true,
            ..NetworkConfig::default()
        }
    }

    #[test]
    fn rates() {
        let _lang = lang(Lang::En);
        assert_eq!(format_rate(0, Unit::Bytes), "0 B/s");
        assert_eq!(format_rate(512, Unit::Bytes), "512 B/s");
        assert_eq!(format_rate(2_300_000, Unit::Bytes), "2.3 MB/s");
        assert_eq!(format_rate(120_000, Unit::Bytes), "120 KB/s");
        assert_eq!(format_rate(12_500_000, Unit::Bytes), "12 MB/s");
        assert_eq!(format_rate(999_800, Unit::Bytes), "1.0 MB/s");
        assert_eq!(format_rate(2_300_000, Unit::Bits), "18 Mb/s");
        assert_eq!(format_rate(125_000, Unit::Bits), "1.0 Mb/s");
        assert_eq!(
            compact_text(2_300_000, 120_000, Unit::Bytes),
            "↓ 2.3 MB/s   ↑ 120 KB/s"
        );
        bw_i18n::set(Lang::Fr);
        assert_eq!(format_rate(2_300_000, Unit::Bytes), "2,3 Mo/s");
        assert_eq!(format_rate(2_300_000, Unit::Bits), "18 Mb/s");
    }

    #[test]
    fn aggregation() {
        let samples = [
            ("Ethernet", 1000, 100),
            ("Wi-Fi", 500, 50),
            ("Loopback Pseudo-Interface 1", 9000, 9000),
        ];
        assert_eq!(aggregate(&samples, false), (1500, 150));
        // Avec un VPN, le tunnel et la carte réelle comptent les mêmes octets.
        assert_eq!(aggregate(&samples, true), (1000, 100));
        assert_eq!(aggregate(&[], false), (0, 0));
    }

    #[test]
    fn ticker_modes() {
        let mut off = Ticker::new(CompactSpeed::Off, 50_000);
        assert!(!off.feed(10_000_000, 0));
        let mut always = Ticker::new(CompactSpeed::Always, 50_000);
        assert!(always.feed(0, 0));

        let mut active = Ticker::new(CompactSpeed::Active, 50_000);
        assert!(!active.feed(1_000, 1_000));
        assert!(active.feed(80_000, 0));
        // Trafic en dents de scie : reste affiché trois relevés calmes.
        assert!(active.feed(0, 0));
        assert!(active.feed(0, 0));
        assert!(!active.feed(0, 0));
        // Un pic pendant l'attente remet le compteur à zéro.
        assert!(active.feed(0, 90_000));
        assert!(active.feed(0, 0));
        assert!(active.feed(0, 90_000));
        assert!(active.feed(0, 0));
        assert!(active.feed(0, 0));
        assert!(!active.feed(0, 0));
    }

    #[test]
    fn connection_notices() {
        let _lang = lang(Lang::En);
        let on = wifi("Home", false);
        let c = config();

        let lost = link_notice(&on, &Link::offline(), &c).unwrap();
        assert_eq!(lost.text, "No internet connection");
        assert_eq!(lost.attention, Attention::Urgent);
        let back = link_notice(&Link::offline(), &on, &c).unwrap();
        assert_eq!(back.text, "Back online");

        let vpn_up = link_notice(&on, &wifi("Home", true), &c).unwrap();
        assert_eq!(vpn_up.text, "VPN connected");
        assert_eq!(vpn_up.attention, Attention::High);
        let vpn_down = link_notice(&wifi("Home", true), &on, &c).unwrap();
        assert_eq!(vpn_down.text, "VPN disconnected");
        assert_eq!(vpn_down.attention, Attention::Urgent);

        let moved = link_notice(&on, &wifi("Office", false), &c).unwrap();
        assert_eq!(moved.text, "Connected to Office");
        // Pas d'annonce si rien ne change, ni pour le simple signal.
        assert_eq!(link_notice(&on, &on, &c), None);
        let mut weaker = on.clone();
        weaker.bars = Some(1);
        assert_eq!(link_notice(&on, &weaker, &c), None);
    }

    #[test]
    fn notices_follow_the_config() {
        let on = wifi("Home", false);
        let quiet = NetworkConfig {
            announce_connection: false,
            announce_vpn: false,
            announce_wifi: false,
            ..NetworkConfig::default()
        };
        assert!(!quiet.announces());
        assert_eq!(link_notice(&on, &Link::offline(), &quiet), None);
        assert_eq!(link_notice(&on, &wifi("Home", true), &quiet), None);
        assert_eq!(link_notice(&on, &wifi("Office", false), &quiet), None);
        // Le Wi-Fi n'est annoncé que sur demande.
        assert_eq!(
            link_notice(&on, &wifi("Office", false), &NetworkConfig::default()),
            None
        );
    }

    #[test]
    fn snapshot_texts() {
        let _lang = lang(Lang::En);
        let snapshot = |link: Option<Link>| NetworkSnapshot {
            down: 2_300_000,
            up: 120_000,
            link,
            unit: Unit::Bytes,
        };
        let home = snapshot(Some(wifi("Home", false)));
        assert_eq!(home.icon(), "wifi");
        assert_eq!(home.detail(), "Home · ↓ 2.3 MB/s  ↑ 120 KB/s");
        assert!(!home.offline());

        let vpn = snapshot(Some(wifi("Home", true)));
        assert_eq!(vpn.icon(), "shield");
        assert_eq!(vpn.detail(), "Home · VPN · ↓ 2.3 MB/s  ↑ 120 KB/s");

        let off = snapshot(Some(Link::offline()));
        assert_eq!(off.icon(), "wifi-off");
        assert_eq!(off.detail(), "Offline");
        assert!(off.offline());

        // Hors Windows : seul le débit est connu.
        let unknown = snapshot(None);
        assert_eq!(unknown.icon(), "globe");
        assert_eq!(unknown.detail(), "↓ 2.3 MB/s  ↑ 120 KB/s");
    }

    #[test]
    fn vpn_adapters() {
        // OpenVPN (carte DCO ou TAP), WireGuard, FortiClient, VPN de Windows.
        assert!(looks_like_vpn(53, "OpenVPN Data Channel Offload"));
        assert!(looks_like_vpn(6, "TAP-Windows Adapter V9"));
        assert!(looks_like_vpn(53, "Wintun Userspace Tunnel"));
        assert!(looks_like_vpn(
            6,
            "Fortinet SSL VPN Virtual Ethernet Adapter"
        ));
        assert!(looks_like_vpn(131, "WAN Miniport (IKEv2)"));
        // Tunnels d'IPv6 de Windows, machines virtuelles, cartes ordinaires.
        assert!(!looks_like_vpn(131, "Teredo Tunneling Pseudo-Interface"));
        assert!(!looks_like_vpn(
            131,
            "Microsoft IP-HTTPS Platform Interface"
        ));
        assert!(!looks_like_vpn(6, "VirtualBox Host-Only Ethernet Adapter"));
        assert!(!looks_like_vpn(71, "Intel(R) Wi-Fi 6E AX211 160MHz"));
        assert!(!looks_like_vpn(6, "Ethernet"));
    }

    #[test]
    fn usable_addresses() {
        let ip = |s: &str| s.parse::<std::net::IpAddr>().unwrap();
        assert!(usable_address(ip("10.2.0.2")));
        assert!(usable_address(ip("192.168.1.20")));
        assert!(usable_address(ip("2001:db8::1")));
        // Adresses locales au lien : un VPN déconnecté n'a que celles-là.
        assert!(!usable_address(ip("169.254.24.153")));
        assert!(!usable_address(ip("fe80::1c4f:c482:33f8:3c9a")));
        assert!(!usable_address(ip("127.0.0.1")));
        assert!(!usable_address(ip("::1")));
        assert!(!usable_address(ip("0.0.0.0")));
    }

    #[test]
    fn labels() {
        let _lang = lang(Lang::En);
        assert_eq!(wifi("Home", false).label(), "Home");
        assert_eq!(Link::offline().label(), "Offline");
        let wired = Link {
            medium: Medium::Ethernet,
            ssid: None,
            bars: None,
            ..wifi("", false)
        };
        assert_eq!(wired.label(), "Ethernet");
    }
}
