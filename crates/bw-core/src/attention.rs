use std::collections::HashMap;

/// Niveau d'attention réclamé par un module.
///
/// L'ordre des variantes compte : `Urgent > High > Low > None`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Attention {
    /// Rien à montrer.
    #[default]
    None,
    /// Info de fond (ex. musique en cours).
    Low,
    /// Mérite un coup d'œil (ex. réunion dans 5 min).
    High,
    /// Action requise (ex. Claude attend une réponse).
    Urgent,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Claim {
    level: Attention,
    summary: Option<String>,
}

/// Module qui occupe actuellement la pilule compacte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Winner {
    pub module: String,
    pub level: Attention,
    pub summary: Option<String>,
}

/// Choisit quel module s'affiche en mode compact.
///
/// Le niveau d'attention le plus élevé gagne ; à égalité, c'est l'ordre de
/// priorité configuré (`layout.compact`) qui tranche. Les modules absents de
/// cette liste passent après, par ordre alphabétique pour rester déterministe.
#[derive(Debug, Default)]
pub struct Arbiter {
    priority: Vec<String>,
    claims: HashMap<String, Claim>,
}

impl Arbiter {
    pub fn new(priority: Vec<String>) -> Self {
        Self {
            priority,
            claims: HashMap::new(),
        }
    }

    /// Met à jour l'ordre de priorité. Retourne `true` si le gagnant a changé.
    pub fn set_priority(&mut self, priority: Vec<String>) -> bool {
        let before = self.winner();
        self.priority = priority;
        before != self.winner()
    }

    /// Enregistre la demande d'un module. Retourne `true` si le gagnant a changé.
    pub fn claim(&mut self, module: &str, level: Attention, summary: Option<String>) -> bool {
        let before = self.winner();
        if level == Attention::None {
            self.claims.remove(module);
        } else {
            self.claims
                .insert(module.to_owned(), Claim { level, summary });
        }
        before != self.winner()
    }

    pub fn winner(&self) -> Option<Winner> {
        self.claims
            .iter()
            .max_by(|(a_id, a), (b_id, b)| {
                a.level
                    .cmp(&b.level)
                    // Rang plus petit = plus prioritaire, donc comparaison inversée.
                    .then_with(|| self.rank(b_id).cmp(&self.rank(a_id)))
            })
            .map(|(id, claim)| Winner {
                module: id.clone(),
                level: claim.level,
                summary: claim.summary.clone(),
            })
    }

    fn rank<'a>(&self, module: &'a str) -> (usize, &'a str) {
        let pos = self
            .priority
            .iter()
            .position(|p| p == module)
            .unwrap_or(usize::MAX);
        (pos, module)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arbiter() -> Arbiter {
        Arbiter::new(vec!["claude".into(), "media".into(), "calendar".into()])
    }

    fn winner_id(a: &Arbiter) -> Option<String> {
        a.winner().map(|w| w.module)
    }

    #[test]
    fn empty_has_no_winner() {
        assert_eq!(arbiter().winner(), None);
    }

    #[test]
    fn highest_level_wins_regardless_of_priority() {
        let mut a = arbiter();
        a.claim("claude", Attention::Low, None);
        a.claim("calendar", Attention::Urgent, None);
        assert_eq!(winner_id(&a).as_deref(), Some("calendar"));
    }

    #[test]
    fn priority_breaks_ties() {
        let mut a = arbiter();
        a.claim("calendar", Attention::High, None);
        a.claim("media", Attention::High, None);
        assert_eq!(winner_id(&a).as_deref(), Some("media"));
    }

    #[test]
    fn unknown_modules_come_last_alphabetically() {
        let mut a = arbiter();
        a.claim("zeta", Attention::Low, None);
        a.claim("alpha", Attention::Low, None);
        assert_eq!(winner_id(&a).as_deref(), Some("alpha"));
        a.claim("calendar", Attention::Low, None);
        assert_eq!(winner_id(&a).as_deref(), Some("calendar"));
    }

    #[test]
    fn none_releases_the_claim() {
        let mut a = arbiter();
        a.claim("claude", Attention::Urgent, None);
        a.claim("media", Attention::Low, None);
        assert!(a.claim("claude", Attention::None, None));
        assert_eq!(winner_id(&a).as_deref(), Some("media"));
    }

    #[test]
    fn claim_reports_winner_changes_only() {
        let mut a = arbiter();
        assert!(a.claim("media", Attention::Low, Some("Song A".into())));
        // Un module moins prioritaire au même niveau ne change rien.
        assert!(!a.claim("calendar", Attention::Low, None));
        // Le résumé du gagnant qui change compte comme un changement.
        assert!(a.claim("media", Attention::Low, Some("Song B".into())));
    }

    #[test]
    fn reordering_priority_can_change_winner() {
        let mut a = arbiter();
        a.claim("media", Attention::Low, None);
        a.claim("calendar", Attention::Low, None);
        assert!(a.set_priority(vec!["calendar".into(), "media".into()]));
        assert_eq!(winner_id(&a).as_deref(), Some("calendar"));
    }
}
