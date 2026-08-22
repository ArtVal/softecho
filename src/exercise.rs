use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExercisePack {
    pub title: String,
    pub exercises: Vec<Exercise>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Exercise {
    ChooseWord {
        prompt: String,
        options: Vec<String>,
        answer: String,
    },
    BuildPhrase {
        prompt: String,
        words: Vec<String>,
        answer: String,
    },
    ReadAloud {
        prompt: String,
        text: String,
    },
}

impl Exercise {
    pub fn prompt(&self) -> &str {
        match self {
            Self::ChooseWord { prompt, .. }
            | Self::BuildPhrase { prompt, .. }
            | Self::ReadAloud { prompt, .. } => prompt,
        }
    }

    /// Целевая фраза для сравнения с голосом (фаза 2).
    pub fn target_text(&self) -> Option<&str> {
        match self {
            Self::ChooseWord { answer, .. } | Self::BuildPhrase { answer, .. } => Some(answer),
            Self::ReadAloud { text, .. } => Some(text),
        }
    }
}

#[derive(Debug, Clone)]
pub enum UserAnswer {
    Choice(String),
    Phrase(Vec<String>),
    /// Самопроверка или результат ASR.
    ReadDone { matched: bool, heard: Option<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckResult {
    Correct,
    Incorrect,
}

pub fn normalize_phrase(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn check_answer(exercise: &Exercise, answer: &UserAnswer) -> CheckResult {
    match (exercise, answer) {
        (Exercise::ChooseWord { answer: expected, .. }, UserAnswer::Choice(got)) => {
            if normalize_phrase(got) == normalize_phrase(expected) {
                CheckResult::Correct
            } else {
                CheckResult::Incorrect
            }
        }
        (Exercise::BuildPhrase { answer: expected, .. }, UserAnswer::Phrase(parts)) => {
            let got = parts.join(" ");
            if normalize_phrase(&got) == normalize_phrase(expected) {
                CheckResult::Correct
            } else {
                CheckResult::Incorrect
            }
        }
        (Exercise::ReadAloud { .. }, UserAnswer::ReadDone { matched, .. }) => {
            if *matched {
                CheckResult::Correct
            } else {
                CheckResult::Incorrect
            }
        }
        _ => CheckResult::Incorrect,
    }
}

/// Сравнение распознанной речи с образцом (допускаем полное совпадение после нормализации).
pub fn speech_matches(target: &str, heard: &str) -> bool {
    normalize_phrase(target) == normalize_phrase(heard)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Progress {
    pub sessions_completed: u32,
    pub total_correct: u32,
    pub total_answered: u32,
}

impl Progress {
    pub fn record_session(&mut self, correct: u32, total: u32) {
        self.sessions_completed += 1;
        self.total_correct += correct;
        self.total_answered += total;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_and_match() {
        assert_eq!(normalize_phrase("  Я  Пью  Воду "), "я пью воду");
        assert_eq!(normalize_phrase("Спасибо, большое!"), "спасибо большое");
        assert!(speech_matches("Доброе утро", "доброе   утро"));
        assert!(speech_matches("Спасибо большое", "спасибо, большое!"));
        assert!(!speech_matches("Доброе утро", "добрый вечер"));
    }

    #[test]
    fn check_choose_word() {
        let ex = Exercise::ChooseWord {
            prompt: "q".into(),
            options: vec!["чай".into(), "стол".into()],
            answer: "чай".into(),
        };
        assert_eq!(
            check_answer(&ex, &UserAnswer::Choice("Чай".into())),
            CheckResult::Correct
        );
        assert_eq!(
            check_answer(&ex, &UserAnswer::Choice("стол".into())),
            CheckResult::Incorrect
        );
        assert_eq!(
            check_answer(&ex, &UserAnswer::Phrase(vec!["чай".into()])),
            CheckResult::Incorrect
        );
    }

    #[test]
    fn check_build_phrase() {
        let ex = Exercise::BuildPhrase {
            prompt: "q".into(),
            words: vec!["Я".into(), "пью".into(), "воду".into()],
            answer: "Я пью воду".into(),
        };
        assert_eq!(
            check_answer(
                &ex,
                &UserAnswer::Phrase(vec!["Я".into(), "пью".into(), "воду".into()])
            ),
            CheckResult::Correct
        );
        assert_eq!(
            check_answer(
                &ex,
                &UserAnswer::Phrase(vec!["пью".into(), "Я".into(), "воду".into()])
            ),
            CheckResult::Incorrect
        );
    }

    #[test]
    fn check_read_aloud_self() {
        let ex = Exercise::ReadAloud {
            prompt: "q".into(),
            text: "Доброе утро".into(),
        };
        assert_eq!(
            check_answer(
                &ex,
                &UserAnswer::ReadDone {
                    matched: true,
                    heard: None
                }
            ),
            CheckResult::Correct
        );
        assert_eq!(
            check_answer(
                &ex,
                &UserAnswer::ReadDone {
                    matched: false,
                    heard: Some("нет".into())
                }
            ),
            CheckResult::Incorrect
        );
    }

    #[test]
    fn progress_accumulates() {
        let mut p = Progress::default();
        p.record_session(3, 5);
        p.record_session(2, 4);
        assert_eq!(p.sessions_completed, 2);
        assert_eq!(p.total_correct, 5);
        assert_eq!(p.total_answered, 9);
    }
}
