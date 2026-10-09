//! Logique pure : quelles notifications sont nouvelles, lesquelles garder,
//! et l'allure de chacune (couleur, initiale). Aucun appel système ici.
// Hors Windows, seuls les tests se servent de la boîte de réception.
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::SystemTime;

use crate::config::NotifyConfig;

/// Notifications gardées pour l'onglet de l'île.
const KEPT: usize = 20;

/// Une notification lue dans le centre de notifications de Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Identifiant attribué par Windows.
    pub id: u32,
    /// Nom affiché de l'application (« Discord »).
    pub app: String,
    /// AppUserModelID, pour rouvrir l'application.
    pub app_id: String,
    /// Première ligne (souvent l'expéditeur).
    pub title: String,
    /// Le reste du message, sur une ligne.
    pub body: String,
    /// Textes d'origine, gardés même quand l'affichage les masque : ils
    /// servent à retrouver la notification dans le centre de notifications.
    pub lines: Vec<String>,
    /// Icône de l'application, si Windows en fournit une.
    pub icon: Option<AppIcon>,
    pub at: SystemTime,
}

/// Icône d'application, RGBA non prémultiplié.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
    /// Couleur dominante de l'icône.
    pub accent: Option<[u8; 3]>,
}

impl Notification {
    /// Couleur de l'application : celle de la marque si on la connaît, sinon
    /// celle de son icône, sinon une teinte stable tirée de son nom.
    pub fn color(&self) -> [u8; 3] {
        known_brand(&self.app)
            .or_else(|| self.icon.as_ref()?.accent)
            .unwrap_or_else(|| brand_color(&self.app))
    }

    /// Lettre de l'avatar.
    pub fn initial(&self) -> String {
        self.app
            .chars()
            .find(|c| c.is_alphanumeric())
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_else(|| "•".into())
    }
}

/// État publié vers l'UI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotifySnapshot {
    /// Plus récente d'abord.
    pub recent: Vec<Notification>,
    /// Notification annoncée dans la pilule en ce moment.
    pub announcing: Option<Notification>,
    /// Arrivées depuis la dernière consultation de l'onglet.
    pub unread: usize,
}

#[derive(Debug, Default)]
pub struct Inbox {
    /// `None` avant la première lecture : ce qui est déjà là n'est pas annoncé.
    known: Option<BTreeSet<u32>>,
    recent: Vec<Notification>,
    unread: usize,
}

impl Inbox {
    /// Prend la liste actuelle du système ; retourne les nouvelles
    /// notifications à annoncer, la plus récente en dernier.
    pub fn sync(&mut self, current: Vec<Notification>, config: &NotifyConfig) -> Vec<Notification> {
        let ids: BTreeSet<u32> = current.iter().map(|n| n.id).collect();
        let mut kept: Vec<Notification> = current
            .into_iter()
            .filter(|n| !config.ignores(&n.app))
            .collect();
        kept.sort_by_key(|n| std::cmp::Reverse(n.id));
        kept.truncate(KEPT);

        let fresh: Vec<Notification> = match &self.known {
            None => Vec::new(),
            Some(known) => kept
                .iter()
                .rev()
                .filter(|n| !known.contains(&n.id))
                .cloned()
                .collect(),
        };
        self.known = Some(ids);
        self.unread = (self.unread + fresh.len()).min(kept.len());
        self.recent = kept;
        fresh
    }

    pub fn mark_seen(&mut self) {
        self.unread = 0;
    }

    /// Retire une notification (effacée depuis l'île).
    pub fn remove(&mut self, id: u32) {
        self.recent.retain(|n| n.id != id);
        self.unread = self.unread.min(self.recent.len());
    }

    pub fn clear(&mut self) {
        self.recent.clear();
        self.unread = 0;
    }

    pub fn snapshot(&self, announcing: Option<Notification>) -> NotifySnapshot {
        NotifySnapshot {
            recent: self.recent.clone(),
            announcing,
            unread: self.unread,
        }
    }
}

/// Texte de la pilule : « Discord · Arkyan ».
pub fn summary(n: &Notification) -> String {
    if n.title.is_empty() || n.title == n.app {
        n.app.clone()
    } else {
        format!("{} · {}", n.app, n.title)
    }
}

/// Couleurs des applications de messagerie courantes, choisies pour rester
/// lisibles sur un fond sombre.
const BRANDS: &[(&str, [u8; 3])] = &[
    ("discord", [0x58, 0x65, 0xF2]),
    ("slack", [0xE0, 0x1E, 0x5A]),
    ("teams", [0x7B, 0x83, 0xEB]),
    ("whatsapp", [0x25, 0xD3, 0x66]),
    ("telegram", [0x2A, 0xAB, 0xEE]),
    ("signal", [0x3A, 0x76, 0xF0]),
    ("messenger", [0x00, 0x84, 0xFF]),
    ("instagram", [0xE1, 0x30, 0x6C]),
    ("outlook", [0x00, 0x78, 0xD4]),
    ("mail", [0x00, 0x78, 0xD4]),
    ("thunderbird", [0x0A, 0x84, 0xFF]),
    ("chrome", [0x42, 0x85, 0xF4]),
    ("edge", [0x2E, 0xB6, 0xEA]),
    ("firefox", [0xFF, 0x71, 0x39]),
    ("spotify", [0x1D, 0xB9, 0x54]),
    ("steam", [0x66, 0xC0, 0xF4]),
    ("zoom", [0x2D, 0x8C, 0xFF]),
    ("skype", [0x00, 0xAF, 0xF0]),
    ("github", [0x8B, 0x94, 0x9E]),
];

fn known_brand(app: &str) -> Option<[u8; 3]> {
    let lower = app.to_lowercase();
    BRANDS
        .iter()
        .find(|(name, _)| lower.contains(name))
        .map(|(_, c)| *c)
}

pub fn brand_color(app: &str) -> [u8; 3] {
    if let Some(c) = known_brand(app) {
        return c;
    }
    // Teinte stable (FNV-1a), saturation et luminosité fixes.
    let hash = app.to_lowercase().bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    hsl(hash % 360, 0.62, 0.58)
}

/// Le texte d'un élément du centre de notifications (« Titre. Message. .
/// Reçu à 18:21 ») est-il celui de la notification aux lignes `lines` ?
pub fn matches_center_entry(entry: &str, lines: &[String]) -> bool {
    let normalize = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let expected = normalize(&lines.join(". "));
    !expected.is_empty() && normalize(entry).starts_with(&expected)
}

/// Pixels BGRA prémultipliés (bitmap Windows) → RGBA ordinaire. Une image
/// sans aucune transparence renseignée est considérée opaque.
pub fn straight_rgba(bgra: &[u8]) -> Vec<u8> {
    let pixels = bgra.as_chunks::<4>().0;
    let opaque = pixels.iter().all(|px| px[3] == 0);
    pixels
        .iter()
        .flat_map(|px| {
            let a = if opaque { 255 } else { px[3] };
            let un = |c: u8| match a {
                0 => 0,
                255 => c,
                _ => ((u32::from(c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8,
            };
            [un(px[2]), un(px[1]), un(px[0]), a]
        })
        .collect()
}

fn hsl(hue: u32, s: f32, l: f32) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h = hue as f32 / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match hue / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [r, g, b].map(|v| ((v + m) * 255.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notif(id: u32, app: &str) -> Notification {
        Notification {
            id,
            app: app.into(),
            app_id: String::new(),
            title: format!("t{id}"),
            body: String::new(),
            lines: vec![format!("t{id}")],
            icon: None,
            at: SystemTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn existing_notifications_are_not_announced() {
        let config = NotifyConfig::default();
        let mut inbox = Inbox::default();
        assert!(
            inbox
                .sync(vec![notif(1, "A"), notif(2, "B")], &config)
                .is_empty()
        );
        assert_eq!(inbox.snapshot(None).recent.len(), 2);

        let fresh = inbox.sync(
            vec![notif(1, "A"), notif(2, "B"), notif(4, "D"), notif(3, "C")],
            &config,
        );
        // La plus récente en dernier : c'est elle qui reste affichée.
        assert_eq!(fresh.iter().map(|n| n.id).collect::<Vec<_>>(), [3, 4]);
        let snap = inbox.snapshot(None);
        assert_eq!(snap.unread, 2);
        assert_eq!(
            snap.recent.iter().map(|n| n.id).collect::<Vec<_>>(),
            [4, 3, 2, 1]
        );

        // Effacée dans Windows : elle quitte la liste, sans être réannoncée.
        assert!(inbox.sync(vec![notif(4, "D")], &config).is_empty());
        assert_eq!(inbox.snapshot(None).unread, 1);
        inbox.mark_seen();
        assert_eq!(inbox.snapshot(None).unread, 0);
    }

    #[test]
    fn remove_and_clear() {
        let config = NotifyConfig::default();
        let mut inbox = Inbox::default();
        inbox.sync(Vec::new(), &config);
        inbox.sync(vec![notif(1, "A"), notif(2, "B")], &config);
        assert_eq!(inbox.snapshot(None).unread, 2);
        inbox.remove(2);
        let snap = inbox.snapshot(None);
        assert_eq!(snap.recent.len(), 1);
        assert_eq!(snap.unread, 1);
        inbox.clear();
        assert_eq!(inbox.snapshot(None), NotifySnapshot::default());
    }

    #[test]
    fn ignored_apps_are_dropped() {
        let config = NotifyConfig {
            ignore: vec!["docker".into()],
            ..NotifyConfig::default()
        };
        let mut inbox = Inbox::default();
        inbox.sync(Vec::new(), &config);
        let fresh = inbox.sync(
            vec![notif(1, "Docker Desktop"), notif(2, "Discord")],
            &config,
        );
        assert_eq!(fresh.len(), 1);
        assert_eq!(inbox.snapshot(None).recent.len(), 1);
    }

    #[test]
    fn icon_color_for_unknown_apps() {
        let mut n = notif(1, "Inconnue");
        n.icon = Some(AppIcon {
            width: 1,
            height: 1,
            rgba: Arc::new(vec![1, 2, 3, 255]),
            accent: Some([9, 8, 7]),
        });
        assert_eq!(n.color(), [9, 8, 7]);
        // Une marque connue garde sa couleur.
        n.app = "Discord".into();
        assert_eq!(n.color(), [0x58, 0x65, 0xF2]);
    }

    #[test]
    fn center_entries() {
        let lines = vec!["Sonde UIA".to_owned(), "message de sonde 42".to_owned()];
        assert!(matches_center_entry(
            "Sonde UIA. message de sonde 42. . Reçu à 18:21",
            &lines
        ));
        assert!(matches_center_entry(
            "Sonde  UIA.\nmessage de sonde 42. . Reçu à 18:21",
            &lines
        ));
        assert!(!matches_center_entry(
            "Autre. message. . Reçu à 18:21",
            &lines
        ));
        assert!(!matches_center_entry("n'importe quoi", &[]));
    }

    #[test]
    fn pixels() {
        // Bleu à moitié transparent, prémultiplié → bleu plein, alpha 128.
        assert_eq!(straight_rgba(&[128, 0, 0, 128]), [0, 0, 255, 128]);
        assert_eq!(
            straight_rgba(&[0, 0, 0, 0, 10, 20, 30, 255]),
            [0, 0, 0, 0, 30, 20, 10, 255]
        );
        // Sans alpha du tout : opaque.
        assert_eq!(straight_rgba(&[10, 20, 30, 0]), [30, 20, 10, 255]);
    }

    #[test]
    fn looks() {
        assert_eq!(brand_color("Discord"), [0x58, 0x65, 0xF2]);
        assert_eq!(brand_color("Inconnue"), brand_color("inconnue"));
        assert_eq!(notif(1, "discord").initial(), "D");
        assert_eq!(notif(1, "  ").initial(), "•");
        assert_eq!(summary(&notif(1, "Discord")), "Discord · t1");
        assert_eq!(hsl(0, 1.0, 0.5), [255, 0, 0]);
        assert_eq!(hsl(120, 1.0, 0.5), [0, 255, 0]);
    }
}
