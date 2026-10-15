//! Réponse en cours à une question de Claude (`AskUserQuestion`) : une
//! question à la fois, un clic répond (choix simple) ou coche (choix
//! multiple). Logique pure ; le contrôleur affiche et envoie.

use bw_claude::Question;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answering {
    /// Demande à laquelle on répond.
    pub id: u64,
    /// Question affichée.
    pub index: usize,
    /// Par question, les rangs des choix retenus.
    pub picks: Vec<Vec<usize>>,
}

impl Answering {
    pub fn new(id: u64, questions: &[Question]) -> Self {
        Self {
            id,
            index: 0,
            picks: vec![Vec::new(); questions.len()],
        }
    }

    /// Clic sur un choix. `Some(action)` quand toutes les questions ont une
    /// réponse : l'action à envoyer au module (`answer:<id>:<choix>`).
    pub fn pick(&mut self, questions: &[Question], option: usize) -> Option<String> {
        let question = questions.get(self.index)?;
        if option >= question.options.len() {
            return None;
        }
        let picks = &mut self.picks[self.index];
        if question.multi {
            match picks.iter().position(|&p| p == option) {
                Some(pos) => {
                    picks.remove(pos);
                }
                None => {
                    picks.push(option);
                    picks.sort_unstable();
                }
            }
            None
        } else {
            *picks = vec![option];
            self.advance(questions.len())
        }
    }

    /// « Suivant » / « Envoyer » d'un choix multiple : seulement si au moins
    /// une option est cochée.
    pub fn next(&mut self, questions: &[Question]) -> Option<String> {
        if self.picks.get(self.index)?.is_empty() {
            return None;
        }
        self.advance(questions.len())
    }

    fn advance(&mut self, count: usize) -> Option<String> {
        self.index += 1;
        (self.index >= count).then(|| {
            let picks = self
                .picks
                .iter()
                .map(|p| p.iter().map(usize::to_string).collect::<Vec<_>>().join(","))
                .collect::<Vec<_>>()
                .join(";");
            format!("answer:{}:{picks}", self.id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(options: usize, multi: bool) -> Question {
        Question {
            question: "?".into(),
            options: (0..options).map(|i| format!("o{i}")).collect(),
            multi,
            ..Question::default()
        }
    }

    #[test]
    fn single_choice_answers_at_once() {
        let questions = [q(2, false)];
        let mut a = Answering::new(5, &questions);
        assert_eq!(a.pick(&questions, 1), Some("answer:5:1".into()));
    }

    #[test]
    fn steps_through_questions_and_multi_select() {
        let questions = [q(3, false), q(3, true)];
        let mut a = Answering::new(7, &questions);
        assert_eq!(a.pick(&questions, 2), None);
        assert_eq!(a.index, 1);
        // Rien de coché : « Envoyer » ne fait rien.
        assert_eq!(a.next(&questions), None);
        assert_eq!(a.pick(&questions, 2), None);
        assert_eq!(a.pick(&questions, 0), None);
        assert_eq!(a.pick(&questions, 1), None);
        // Décocher.
        assert_eq!(a.pick(&questions, 1), None);
        assert_eq!(a.picks[1], [0, 2]);
        assert_eq!(a.next(&questions), Some("answer:7:2;0,2".into()));
    }

    #[test]
    fn out_of_range_is_ignored() {
        let questions = [q(2, false)];
        let mut a = Answering::new(1, &questions);
        assert_eq!(a.pick(&questions, 9), None);
        assert_eq!(a.index, 0);
    }
}
