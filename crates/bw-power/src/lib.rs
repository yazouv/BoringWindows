//! Énergie : la batterie du PC (branchement, charge terminée, niveau faible)
//! et les appareils Bluetooth (connexion avec leur niveau de batterie,
//! déconnexion, niveau faible). Tout passe par des événements de Windows,
//! sans interrogation périodique.
//!
//! La logique (`battery_notice`, `bluetooth_notice`) est pure : elle compare
//! deux états successifs et dit quoi annoncer.

mod config;
mod module;
#[cfg(windows)]
mod win_battery;
#[cfg(windows)]
mod win_bluetooth;

use bw_core::Attention;
use bw_i18n::tr;

pub use config::PowerConfig;
pub use module::{BATTERY_ID, BLUETOOTH_ID, PowerModule};

/// Signal envoyé au thread d'écoute Windows.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wake {
    /// Windows signale un changement : relire l'état.
    Changed,
    Stop,
}

/// Jauge affichée dans la pilule avec le texte de l'annonce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gauge {
    /// Icône : `bolt` (en charge), `battery`, `headphones` ou `bluetooth`.
    pub icon: &'static str,
    /// Niveau (%), s'il est connu.
    pub level: Option<u8>,
    pub charging: bool,
    pub low: bool,
}

/// Ce qu'un changement fait apparaître dans la pilule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub gauge: Gauge,
    pub attention: Attention,
}

// --- Batterie du PC ---------------------------------------------------------

/// État de la batterie du PC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    /// Faux sur un PC de bureau : le module ne dit rien.
    pub present: bool,
    /// Secteur branché.
    pub plugged: bool,
    pub percent: u8,
}

/// Annonce pour le passage de `prev` à `now` ; `None` : rien à dire.
pub fn battery_notice(prev: Battery, now: Battery, low: u8) -> Option<Notice> {
    if !now.present {
        return None;
    }
    let gauge = |icon, low| Gauge {
        icon,
        level: Some(now.percent),
        charging: now.plugged,
        low,
    };
    let percent = now.percent;
    if now.plugged && !prev.plugged {
        return Some(Notice {
            text: tr!("Charging · {} %", "En charge · {} %", percent),
            gauge: gauge("bolt", false),
            attention: Attention::High,
        });
    }
    if !now.plugged && prev.plugged {
        return Some(Notice {
            text: tr!("On battery · {} %", "Sur batterie · {} %", percent),
            gauge: gauge("battery", percent <= low),
            attention: Attention::High,
        });
    }
    if now.plugged && percent >= 100 && prev.percent < 100 {
        return Some(Notice {
            text: tr!("Fully charged", "Charge terminée"),
            gauge: gauge("bolt", false),
            attention: Attention::High,
        });
    }
    // Sous le seuil, puis à la moitié du seuil (20 %, puis 10 %).
    let crossed = |limit: u8| prev.percent > limit && percent <= limit;
    if !now.plugged && (crossed(low) || crossed(low / 2)) {
        return Some(Notice {
            text: tr!("Low battery · {} %", "Batterie faible · {} %", percent),
            gauge: gauge("battery", true),
            attention: Attention::Urgent,
        });
    }
    None
}

// --- Appareils Bluetooth ----------------------------------------------------

/// Appareil Bluetooth appairé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub connected: bool,
    /// Niveau de batterie rapporté par Windows, s'il y en a un.
    pub level: Option<u8>,
    /// Casque, écouteurs, enceinte : icône de casque.
    pub audio: bool,
}

impl Device {
    fn gauge(&self, low: bool) -> Gauge {
        Gauge {
            icon: if self.audio {
                "headphones"
            } else {
                "bluetooth"
            },
            level: self.level,
            charging: false,
            low,
        }
    }
}

/// Annonce pour le passage de `prev` à `now` (le premier changement
/// trouvé) ; `None` : rien à dire.
pub fn bluetooth_notice(prev: &[Device], now: &[Device], low: u8) -> Option<Notice> {
    for device in now {
        let before = prev.iter().find(|d| d.id == device.id);
        let was_connected = before.is_some_and(|d| d.connected);
        let level_before = before.and_then(|d| d.level);
        let name = &device.name;

        if device.connected && !was_connected {
            let text = match device.level {
                Some(level) => format!("{name} · {level} %"),
                None => tr!("{} connected", "{} connecté", name),
            };
            return Some(Notice {
                text,
                gauge: device.gauge(device.level.is_some_and(|l| l <= low)),
                attention: Attention::High,
            });
        }
        if !device.connected && was_connected {
            return Some(Notice {
                text: tr!("{} disconnected", "{} déconnecté", name),
                gauge: Gauge {
                    level: None,
                    ..device.gauge(false)
                },
                attention: Attention::High,
            });
        }
        if !device.connected {
            continue;
        }
        match (level_before, device.level) {
            // Le niveau arrive souvent quelques secondes après la connexion.
            (None, Some(level)) => {
                return Some(Notice {
                    text: format!("{name} · {level} %"),
                    gauge: device.gauge(level <= low),
                    attention: Attention::High,
                });
            }
            (Some(before), Some(level)) if before > low && level <= low => {
                return Some(Notice {
                    text: tr!(
                        "{} · low battery {} %",
                        "{} · batterie faible {} %",
                        name,
                        level
                    ),
                    gauge: device.gauge(true),
                    attention: Attention::Urgent,
                });
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bat(plugged: bool, percent: u8) -> Battery {
        Battery {
            present: true,
            plugged,
            percent,
        }
    }

    #[test]
    fn battery_events() {
        bw_i18n::set(bw_i18n::Lang::Fr);
        let n = battery_notice(bat(false, 54), bat(true, 54), 20).unwrap();
        assert_eq!(n.text, "En charge · 54 %");
        assert_eq!(n.gauge.icon, "bolt");
        assert!(n.gauge.charging);

        let n = battery_notice(bat(true, 54), bat(false, 54), 20).unwrap();
        assert_eq!(n.text, "Sur batterie · 54 %");

        let n = battery_notice(bat(true, 99), bat(true, 100), 20).unwrap();
        assert_eq!(n.text, "Charge terminée");

        let n = battery_notice(bat(false, 21), bat(false, 20), 20).unwrap();
        assert_eq!(n.text, "Batterie faible · 20 %");
        assert_eq!(n.attention, Attention::Urgent);
        assert!(battery_notice(bat(false, 20), bat(false, 19), 20).is_none());
        assert!(battery_notice(bat(false, 11), bat(false, 10), 20).is_some());
        assert!(battery_notice(bat(true, 21), bat(true, 20), 20).is_none());
        assert!(battery_notice(bat(false, 60), bat(false, 59), 20).is_none());

        let desktop = Battery {
            present: false,
            plugged: true,
            percent: 100,
        };
        assert!(battery_notice(bat(false, 50), desktop, 20).is_none());
    }

    fn dev(connected: bool, level: Option<u8>) -> Device {
        Device {
            id: "a".into(),
            name: "WH-1000XM5".into(),
            connected,
            level,
            audio: true,
        }
    }

    #[test]
    fn bluetooth_events() {
        bw_i18n::set(bw_i18n::Lang::Fr);
        let n = bluetooth_notice(&[dev(false, None)], &[dev(true, None)], 20).unwrap();
        assert_eq!(n.text, "WH-1000XM5 connecté");
        assert_eq!(n.gauge.icon, "headphones");

        let n = bluetooth_notice(&[dev(true, None)], &[dev(true, Some(80))], 20).unwrap();
        assert_eq!(n.text, "WH-1000XM5 · 80 %");
        assert_eq!(n.gauge.level, Some(80));

        // Appareil apparu déjà connecté (appairage) : annoncé aussi.
        assert!(bluetooth_notice(&[], &[dev(true, Some(80))], 20).is_some());

        let n = bluetooth_notice(&[dev(true, Some(21))], &[dev(true, Some(19))], 20).unwrap();
        assert_eq!(n.text, "WH-1000XM5 · batterie faible 19 %");
        assert_eq!(n.attention, Attention::Urgent);

        let n = bluetooth_notice(&[dev(true, Some(80))], &[dev(false, Some(80))], 20).unwrap();
        assert_eq!(n.text, "WH-1000XM5 déconnecté");

        assert!(bluetooth_notice(&[dev(true, Some(80))], &[dev(true, Some(70))], 20).is_none());
        assert!(bluetooth_notice(&[dev(false, None)], &[dev(false, Some(50))], 20).is_none());
    }
}
