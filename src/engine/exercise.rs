use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExercisePack {
    pub title: String,
    pub exercises: Vec<Exercise>,
}

/// Ступень / уровень занятия. Сессия идёт строго: слоги → слова → фразы.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExerciseStage {
    Syllable,
    Word,
    Phrase,
}

impl ExerciseStage {
    pub const ALL: [ExerciseStage; 3] = [
        ExerciseStage::Syllable,
        ExerciseStage::Word,
        ExerciseStage::Phrase,
    ];

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Syllable => "Слоги",
            Self::Word => "Слова",
            Self::Phrase => "Фразы",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Exercise {
    ChooseWord {
        #[serde(default)]
        stage: Option<ExerciseStage>,
        prompt: String,
        options: Vec<String>,
        answer: String,
    },
    BuildPhrase {
        #[serde(default)]
        stage: Option<ExerciseStage>,
        prompt: String,
        words: Vec<String>,
        answer: String,
    },
    ReadAloud {
        #[serde(default)]
        stage: Option<ExerciseStage>,
        prompt: String,
        text: String,
        /// Что слушать голосом, если на экране короче (слог «МА», вслух «ма ма ма»).
        #[serde(default)]
        speak: Option<String>,
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

    pub fn stage(&self) -> ExerciseStage {
        let explicit = match self {
            Self::ChooseWord { stage, .. }
            | Self::BuildPhrase { stage, .. }
            | Self::ReadAloud { stage, .. } => *stage,
        };
        if let Some(s) = explicit {
            return s;
        }
        match self {
            Self::ChooseWord { .. } => ExerciseStage::Word,
            Self::BuildPhrase { .. } => ExerciseStage::Phrase,
            Self::ReadAloud { text, .. } => {
                if normalize_phrase(text).split_whitespace().count() <= 1 {
                    ExerciseStage::Word
                } else {
                    ExerciseStage::Phrase
                }
            }
        }
    }

    /// Целевая фраза для сравнения с голосом.
    pub fn target_text(&self) -> Option<&str> {
        match self {
            Self::ChooseWord { answer, .. } | Self::BuildPhrase { answer, .. } => Some(answer),
            Self::ReadAloud { speak, text, .. } => Some(speak.as_deref().unwrap_or(text)),
        }
    }
}

/// Внутри ступени — случайный порядок; между ступенями — нет.
pub fn order_session(exercises: Vec<Exercise>) -> Vec<Exercise> {
    use rand::seq::SliceRandom;
    let mut syllables = Vec::new();
    let mut words = Vec::new();
    let mut phrases = Vec::new();
    for ex in exercises {
        match ex.stage() {
            ExerciseStage::Syllable => syllables.push(ex),
            ExerciseStage::Word => words.push(ex),
            ExerciseStage::Phrase => phrases.push(ex),
        }
    }
    let mut rng = rand::rng();
    syllables.shuffle(&mut rng);
    words.shuffle(&mut rng);
    phrases.shuffle(&mut rng);
    syllables.extend(words);
    syllables.extend(phrases);
    syllables
}

/// Занятие с выбранного уровня: от этой ступени и выше (слоги→слова→фразы).
pub fn order_session_for_level(
    exercises: Vec<Exercise>,
    level: ExerciseStage,
) -> Vec<Exercise> {
    let filtered: Vec<_> = exercises
        .into_iter()
        .filter(|e| e.stage() >= level)
        .collect();
    order_session(filtered)
}

/// Короткий набор для экспресс-диагностики: до `per_stage` заданий на ступень.
/// Предпочитаем «прочитать вслух», затем выбор слова, затем сборку фразы.
pub fn build_diagnosis_set(exercises: &[Exercise], per_stage: usize) -> Vec<Exercise> {
    use rand::seq::SliceRandom;
    let mut out = Vec::new();
    for stage in ExerciseStage::ALL {
        let mut pool: Vec<Exercise> = exercises
            .iter()
            .filter(|e| e.stage() == stage)
            .cloned()
            .collect();
        pool.sort_by_key(|e| match e {
            Exercise::ReadAloud { .. } => 0u8,
            Exercise::ChooseWord { .. } => 1,
            Exercise::BuildPhrase { .. } => 2,
        });
        // Внутри одного приоритета типа — чуть перемешать.
        let mut by_prio: Vec<(u8, Vec<Exercise>)> = Vec::new();
        for e in pool {
            let p = match &e {
                Exercise::ReadAloud { .. } => 0u8,
                Exercise::ChooseWord { .. } => 1,
                Exercise::BuildPhrase { .. } => 2,
            };
            if by_prio.last().map(|(k, _)| *k) != Some(p) {
                by_prio.push((p, Vec::new()));
            }
            by_prio.last_mut().unwrap().1.push(e);
        }
        let mut rng = rand::rng();
        let mut stage_pick = Vec::new();
        for (_, mut group) in by_prio {
            group.shuffle(&mut rng);
            stage_pick.extend(group);
        }
        out.extend(stage_pick.into_iter().take(per_stage));
    }
    out
}

/// Уровень = первая ступень, где меньше половины верных; если все ок — «Фразы».
pub fn infer_level(outcomes: &[(ExerciseStage, bool)]) -> ExerciseStage {
    for stage in ExerciseStage::ALL {
        let items: Vec<bool> = outcomes
            .iter()
            .filter(|(s, _)| *s == stage)
            .map(|(_, ok)| *ok)
            .collect();
        if items.is_empty() {
            continue;
        }
        let ok = items.iter().filter(|b| **b).count();
        let need = items.len().div_ceil(2);
        if ok < need {
            return stage;
        }
    }
    ExerciseStage::Phrase
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

/// Сравнение распознанной речи с образцом.
/// Допускаем: совпадение целиком, цель как подстрока, хвост `[unk]` от Vosk.
pub fn speech_matches(target: &str, heard: &str) -> bool {
    let target = normalize_phrase(target);
    let heard = normalize_phrase(
        &heard
            .replace("[unk]", " ")
            .replace("[UNK]", " "),
    );
    if target.is_empty() || heard.is_empty() {
        return false;
    }
    if target == heard {
        return true;
    }
    let t_words: Vec<&str> = target.split_whitespace().collect();
    let h_words: Vec<&str> = heard.split_whitespace().collect();
    // Слог, повторённый для ASR: «ма ма ма» засчитываем, если услышали «ма».
    if !t_words.is_empty() && t_words.iter().all(|w| *w == t_words[0]) {
        return h_words.iter().any(|w| *w == t_words[0]);
    }
    // «доброе утро пожалуйста» при цели «доброе утро»
    h_words.windows(t_words.len().max(1)).any(|w| w == t_words.as_slice())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Progress {
    pub sessions_completed: u32,
    pub total_correct: u32,
    pub total_answered: u32,
    /// Рабочий уровень (слоги / слова / фразы). Ставится диагностикой или вручную.
    #[serde(default)]
    pub level: Option<ExerciseStage>,
}

impl Progress {
    pub fn record_session(&mut self, correct: u32, total: u32) {
        self.sessions_completed += 1;
        self.total_correct += correct;
        self.total_answered += total;
    }

    pub fn set_level(&mut self, level: ExerciseStage) {
        self.level = Some(level);
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
        assert!(speech_matches("Доброе утро", "доброе утро [unk]"));
        assert!(speech_matches("Доброе утро", "ну доброе утро"));
        assert!(!speech_matches("Доброе утро", "добрый вечер"));
        assert!(!speech_matches("Доброе утро", ""));
        assert!(speech_matches("ма ма ма", "ма"));
        assert!(speech_matches("ма", "ма ма"));
        assert!(!speech_matches("ма ма ма", "па"));
    }

    #[test]
    fn session_keeps_stage_order() {
        let pack = vec![
            Exercise::ReadAloud {
                stage: Some(ExerciseStage::Phrase),
                prompt: "p".into(),
                text: "доброе утро".into(),
                speak: None,
            },
            Exercise::ReadAloud {
                stage: Some(ExerciseStage::Syllable),
                prompt: "s".into(),
                text: "МА".into(),
                speak: Some("ма ма ма".into()),
            },
            Exercise::ChooseWord {
                stage: Some(ExerciseStage::Word),
                prompt: "w".into(),
                options: vec!["чай".into(), "стол".into()],
                answer: "чай".into(),
            },
        ];
        let ordered = order_session(pack);
        let stages: Vec<_> = ordered.iter().map(Exercise::stage).collect();
        assert_eq!(
            stages,
            vec![
                ExerciseStage::Syllable,
                ExerciseStage::Word,
                ExerciseStage::Phrase
            ]
        );
        assert_eq!(ordered[0].target_text(), Some("ма ма ма"));
    }

    #[test]
    fn level_filters_lower_stages() {
        let pack = vec![
            Exercise::ReadAloud {
                stage: Some(ExerciseStage::Syllable),
                prompt: "s".into(),
                text: "МА".into(),
                speak: None,
            },
            Exercise::ReadAloud {
                stage: Some(ExerciseStage::Word),
                prompt: "w".into(),
                text: "мама".into(),
                speak: None,
            },
            Exercise::ReadAloud {
                stage: Some(ExerciseStage::Phrase),
                prompt: "p".into(),
                text: "доброе утро".into(),
                speak: None,
            },
        ];
        let ordered = order_session_for_level(pack, ExerciseStage::Word);
        assert!(ordered.iter().all(|e| e.stage() >= ExerciseStage::Word));
        assert_eq!(ordered[0].stage(), ExerciseStage::Word);
        assert_eq!(ordered.last().unwrap().stage(), ExerciseStage::Phrase);
    }

    #[test]
    fn infer_level_first_weak_stage() {
        // 1 из 2 = половина → ступень пройдена; пустые ступени пропускаются.
        assert_eq!(
            infer_level(&[
                (ExerciseStage::Syllable, true),
                (ExerciseStage::Syllable, false),
                (ExerciseStage::Word, true),
            ]),
            ExerciseStage::Phrase
        );
        // 0 из 1 на словах → уровень «Слова».
        assert_eq!(
            infer_level(&[
                (ExerciseStage::Syllable, true),
                (ExerciseStage::Syllable, true),
                (ExerciseStage::Word, false),
                (ExerciseStage::Phrase, true),
            ]),
            ExerciseStage::Word
        );
        // Меньше половины на слогах (0 из 2).
        assert_eq!(
            infer_level(&[
                (ExerciseStage::Syllable, false),
                (ExerciseStage::Syllable, false),
                (ExerciseStage::Word, true),
            ]),
            ExerciseStage::Syllable
        );
        assert_eq!(
            infer_level(&[
                (ExerciseStage::Syllable, true),
                (ExerciseStage::Word, true),
                (ExerciseStage::Phrase, true),
            ]),
            ExerciseStage::Phrase
        );
    }

    #[test]
    fn diagnosis_set_covers_stages() {
        let pack = vec![
            Exercise::ReadAloud {
                stage: Some(ExerciseStage::Syllable),
                prompt: "s".into(),
                text: "МА".into(),
                speak: None,
            },
            Exercise::ChooseWord {
                stage: Some(ExerciseStage::Word),
                prompt: "w".into(),
                options: vec!["а".into(), "б".into()],
                answer: "а".into(),
            },
            Exercise::ReadAloud {
                stage: Some(ExerciseStage::Phrase),
                prompt: "p".into(),
                text: "доброе утро".into(),
                speak: None,
            },
        ];
        let d = build_diagnosis_set(&pack, 2);
        assert_eq!(d.len(), 3);
        assert_eq!(d[0].stage(), ExerciseStage::Syllable);
        assert_eq!(d[1].stage(), ExerciseStage::Word);
        assert_eq!(d[2].stage(), ExerciseStage::Phrase);
    }

    #[test]
    fn check_choose_word() {
        let ex = Exercise::ChooseWord {
            stage: None,
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
            stage: None,
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
            stage: None,
            prompt: "q".into(),
            text: "Доброе утро".into(),
            speak: None,
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
