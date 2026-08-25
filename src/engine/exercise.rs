use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExercisePack {
    pub title: String,
    pub exercises: Vec<Exercise>,
}

/// Ступень / уровень занятия. Сессия: звуки → слоги → слова → фразы → скороговорки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExerciseStage {
    /// Изолированные гласные и согласные (А, О, М…).
    Sound,
    Syllable,
    Word,
    Phrase,
    /// Скороговорки / автоматизация (после фраз).
    Twister,
}

impl ExerciseStage {
    pub const ALL: [ExerciseStage; 5] = [
        ExerciseStage::Sound,
        ExerciseStage::Syllable,
        ExerciseStage::Word,
        ExerciseStage::Phrase,
        ExerciseStage::Twister,
    ];

    /// Ступени экспресс-диагностики (без скороговорок).
    pub const DIAGNOSIS: [ExerciseStage; 4] = [
        ExerciseStage::Sound,
        ExerciseStage::Syllable,
        ExerciseStage::Word,
        ExerciseStage::Phrase,
    ];

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Sound => "Звуки",
            Self::Syllable => "Слоги",
            Self::Word => "Слова",
            Self::Phrase => "Фразы",
            Self::Twister => "Скороговорки",
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

    /// Ключ в карте произнесения (нормализованная цель).
    /// Повтор слога «ма ма ма» и ответ «ма» дают один ключ — иначе карта не склеивает ступени.
    pub fn map_key(&self) -> Option<String> {
        self.target_text().map(speech_map_key)
    }

    /// Подпись для UI карты.
    pub fn map_label(&self) -> Option<String> {
        match self {
            Self::ReadAloud { text, .. } => Some(text.clone()),
            Self::ChooseWord { answer, .. } | Self::BuildPhrase { answer, .. } => {
                Some(answer.clone())
            }
        }
    }
}

fn split_by_stage(exercises: Vec<Exercise>) -> Vec<Vec<Exercise>> {
    let mut buckets: Vec<Vec<Exercise>> = ExerciseStage::ALL.iter().map(|_| Vec::new()).collect();
    for ex in exercises {
        if let Some(i) = ExerciseStage::ALL.iter().position(|s| *s == ex.stage()) {
            buckets[i].push(ex);
        }
    }
    buckets
}

/// Внутри ступени — случайный порядок; между ступенями — нет.
#[allow(dead_code)]
pub fn order_session(exercises: Vec<Exercise>) -> Vec<Exercise> {
    use rand::seq::SliceRandom;
    let mut buckets = split_by_stage(exercises);
    let mut rng = rand::rng();
    let mut out = Vec::new();
    for bucket in &mut buckets {
        bucket.shuffle(&mut rng);
        out.append(bucket);
    }
    out
}

/// Занятие с выбранного уровня: от этой ступени и выше.
/// Скороговорки — только если `include_twister` (разблокировка).
#[allow(dead_code)]
pub fn order_session_for_level(
    exercises: Vec<Exercise>,
    level: ExerciseStage,
) -> Vec<Exercise> {
    order_session_for_level_with_map(exercises, level, &SpeechMap::default(), true)
}

/// То же, но внутри ступени сначала слабые места из карты.
pub fn order_session_for_level_with_map(
    exercises: Vec<Exercise>,
    level: ExerciseStage,
    map: &SpeechMap,
    include_twister: bool,
) -> Vec<Exercise> {
    let filtered: Vec<Exercise> = exercises
        .into_iter()
        .filter(|ex| {
            let s = ex.stage();
            if s == ExerciseStage::Twister {
                include_twister && s >= level
            } else {
                s >= level
            }
        })
        .collect();
    let mut out = Vec::new();
    for bucket in split_by_stage(filtered) {
        out.extend(order_stage_by_map(bucket, map));
    }
    out
}

/// Скороговорки: уровень ≥ «Фразы» или ≥70% «получается» среди фраз набора с попытками.
pub fn twister_unlocked(
    level: Option<ExerciseStage>,
    pack: &ExercisePack,
    map: &SpeechMap,
) -> bool {
    if matches!(
        level,
        Some(ExerciseStage::Phrase | ExerciseStage::Twister)
    ) {
        return true;
    }
    let entries = pack_speech_entries(pack, map);
    let attempted: Vec<_> = entries
        .iter()
        .filter(|e| e.stage == ExerciseStage::Phrase && e.attempts > 0)
        .collect();
    if attempted.is_empty() {
        return false;
    }
    let good = attempted
        .iter()
        .filter(|e| e.rating == SpeechRating::Good)
        .count();
    (good as f32 / attempted.len() as f32) >= 0.70
}

fn rating_priority(r: SpeechRating) -> u8 {
    match r {
        SpeechRating::Weak => 0,
        SpeechRating::Almost => 1,
        SpeechRating::Unknown => 2,
        SpeechRating::Good => 3,
    }
}

fn order_stage_by_map(mut exercises: Vec<Exercise>, map: &SpeechMap) -> Vec<Exercise> {
    use rand::seq::SliceRandom;
    if exercises.is_empty() {
        return exercises;
    }
    exercises.sort_by_key(|e| {
        e.map_key()
            .map(|k| rating_priority(map.rating(&k)))
            .unwrap_or(2)
    });
    let mut rng = rand::rng();
    let mut buckets: Vec<(u8, Vec<Exercise>)> = Vec::new();
    for ex in exercises {
        let p = ex
            .map_key()
            .map(|k| rating_priority(map.rating(&k)))
            .unwrap_or(2);
        if buckets.last().map(|(k, _)| *k) != Some(p) {
            buckets.push((p, Vec::new()));
        }
        buckets.last_mut().unwrap().1.push(ex);
    }
    let mut out = Vec::new();
    for (_, mut group) in buckets {
        group.shuffle(&mut rng);
        out.extend(group);
    }
    out
}

/// Короткий набор для экспресс-диагностики: до `per_stage` заданий на ступень.
/// Предпочитаем «прочитать вслух», затем выбор слова, затем сборку фразы.
pub fn build_diagnosis_set(exercises: &[Exercise], per_stage: usize) -> Vec<Exercise> {
    use rand::seq::SliceRandom;
    let mut out = Vec::new();
    for stage in ExerciseStage::DIAGNOSIS {
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
    for stage in ExerciseStage::DIAGNOSIS {
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

/// Ключ карты: нормализация + свёртка повторов («ма ма ма» → «ма»).
pub fn speech_map_key(target: &str) -> String {
    let n = normalize_phrase(target);
    let words: Vec<&str> = n.split_whitespace().collect();
    if !words.is_empty() && words.iter().all(|w| *w == words[0]) {
        words[0].to_string()
    } else {
        n
    }
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

/// Имя русской буквы, если распознаватель назвал согласный, а не звук.
fn russian_letter_name(sound: &str) -> Option<&'static str> {
    Some(match sound {
        "б" => "бэ",
        "в" => "вэ",
        "г" => "гэ",
        "д" => "дэ",
        "ж" => "жэ",
        "з" => "зэ",
        "к" => "ка",
        "л" => "эль",
        "м" => "эм",
        "н" => "эн",
        "п" => "пэ",
        "р" => "эр",
        "с" => "эс",
        "т" => "тэ",
        "ф" => "эф",
        "х" => "ха",
        "ц" => "цэ",
        "ч" => "че",
        "ш" => "ша",
        "щ" => "ща",
        _ => return None,
    })
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
    // Повтор для ASR: «ма ма ма» / «а а а» засчитываем, если услышали единицу.
    if !t_words.is_empty() && t_words.iter().all(|w| *w == t_words[0]) {
        let unit = t_words[0];
        if h_words.contains(&unit) {
            return true;
        }
        // Согласный: Vosk часто даёт имя буквы («эм» вместо «м»).
        if let Some(name) = russian_letter_name(unit) {
            return h_words.contains(&name);
        }
        return false;
    }
    // «доброе утро пожалуйста» при цели «доброе утро»
    h_words.windows(t_words.len().max(1)).any(|w| w == t_words.as_slice())
}

/// Оценка произнесения одной цели (звук / слог / слово / фраза).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeechRating {
    Unknown,
    Good,
    Almost,
    Weak,
}

impl SpeechRating {
    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Unknown => "ещё не пробовали",
            Self::Good => "получается",
            Self::Almost => "почти",
            Self::Weak => "нужна практика",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WordStat {
    pub correct: u32,
    pub attempts: u32,
}

impl WordStat {
    pub fn rating(&self) -> SpeechRating {
        if self.attempts == 0 {
            return SpeechRating::Unknown;
        }
        let ratio = self.correct as f32 / self.attempts as f32;
        if ratio >= 0.75 {
            SpeechRating::Good
        } else if ratio >= 0.35 {
            SpeechRating::Almost
        } else {
            SpeechRating::Weak
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpeechMap {
    #[serde(default)]
    pub items: HashMap<String, WordStat>,
}

impl SpeechMap {
    pub fn record(&mut self, key: &str, correct: bool) {
        let key = speech_map_key(key);
        let stat = self.items.entry(key).or_default();
        stat.attempts += 1;
        if correct {
            stat.correct += 1;
        }
    }

    pub fn rating(&self, key: &str) -> SpeechRating {
        let key = speech_map_key(key);
        self.items
            .get(&key)
            .map(WordStat::rating)
            .unwrap_or(SpeechRating::Unknown)
    }

    /// Склеить старые ключи «ма ма ма» с каноническим «ма» (после обновления).
    pub fn normalize_keys(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let mut merged: HashMap<String, WordStat> = HashMap::new();
        for (k, v) in std::mem::take(&mut self.items) {
            let nk = speech_map_key(&k);
            let e = merged.entry(nk).or_default();
            e.correct = e.correct.saturating_add(v.correct);
            e.attempts = e.attempts.saturating_add(v.attempts);
            if e.correct > e.attempts {
                e.correct = e.attempts;
            }
        }
        self.items = merged;
    }
}

#[derive(Debug, Clone)]
pub struct SpeechMapEntry {
    pub label: String,
    pub stage: ExerciseStage,
    pub rating: SpeechRating,
    pub correct: u32,
    pub attempts: u32,
}

/// Уникальные цели текущего набора + статистика из карты.
/// Сначала звуки, потом слоги, слова, фразы (чтобы заголовки UI не прыгали).
pub fn pack_speech_entries(pack: &ExercisePack, map: &SpeechMap) -> Vec<SpeechMapEntry> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for stage in ExerciseStage::ALL {
        for ex in pack.exercises.iter().filter(|e| e.stage() == stage) {
            let Some(key) = ex.map_key() else {
                continue;
            };
            if !seen.insert(key.clone()) {
                continue;
            }
            let stat = map.items.get(&key).cloned().unwrap_or_default();
            out.push(SpeechMapEntry {
                label: ex.map_label().unwrap_or(key),
                stage,
                rating: stat.rating(),
                correct: stat.correct,
                attempts: stat.attempts,
            });
        }
    }
    out
}

/// Сводка карты по ступеням (для экрана прогресса).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageSummary {
    pub stage: ExerciseStage,
    pub good: u32,
    pub almost: u32,
    pub weak: u32,
    pub unknown: u32,
}

pub fn speech_map_stage_summaries(entries: &[SpeechMapEntry]) -> Vec<StageSummary> {
    let mut out = Vec::new();
    for stage in ExerciseStage::ALL {
        let mut s = StageSummary {
            stage,
            good: 0,
            almost: 0,
            weak: 0,
            unknown: 0,
        };
        let mut any = false;
        for e in entries.iter().filter(|e| e.stage == stage) {
            any = true;
            match e.rating {
                SpeechRating::Good => s.good += 1,
                SpeechRating::Almost => s.almost += 1,
                SpeechRating::Weak => s.weak += 1,
                SpeechRating::Unknown => s.unknown += 1,
            }
        }
        if any {
            out.push(s);
        }
    }
    out
}

/// Короткий текстовый отчёт (скопировать родственнику / логопеду).
pub fn format_progress_report(
    progress: &Progress,
    pack_title: &str,
    entries: &[SpeechMapEntry],
) -> String {
    let mut lines = Vec::new();
    lines.push("SoftEcho — отчёт о прогрессе".into());
    lines.push(format!("Набор: {pack_title}"));
    let level = progress
        .level
        .map(|l| l.label_ru().to_string())
        .unwrap_or_else(|| "не выбран".into());
    lines.push(format!("Уровень: {level}"));
    lines.push(format!(
        "Занятий: {} · верно {}/{}",
        progress.sessions_completed, progress.total_correct, progress.total_answered
    ));
    if let Some(acc) = progress.recent_accuracy() {
        lines.push(format!(
            "Тренд (последние {}): {:.0}% верных",
            progress.session_history.len(),
            acc * 100.0
        ));
    }
    if !progress.session_history.is_empty() {
        lines.push("История занятий (старые → новые):".into());
        for (i, s) in progress.session_history.iter().enumerate() {
            let pct = if s.total == 0 {
                0
            } else {
                (100 * s.correct) / s.total
            };
            lines.push(format!("  {}. {}/{} ({}%)", i + 1, s.correct, s.total, pct));
        }
    }
    lines.push("Карта по ступеням:".into());
    for s in speech_map_stage_summaries(entries) {
        lines.push(format!(
            "  {}: получается {}, почти {}, нужна практика {}, ещё нет {}",
            s.stage.label_ru(),
            s.good,
            s.almost,
            s.weak,
            s.unknown
        ));
    }
    let weak: Vec<_> = entries
        .iter()
        .filter(|e| e.rating == SpeechRating::Weak)
        .map(|e| e.label.as_str())
        .collect();
    if !weak.is_empty() {
        lines.push(format!("Слабые места: {}", weak.join(", ")));
    }
    lines.push("Не заменяет занятие с логопедом.".into());
    lines.join("\n")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Progress {
    pub sessions_completed: u32,
    pub total_correct: u32,
    pub total_answered: u32,
    /// Рабочий уровень. Ставится диагностикой или вручную.
    #[serde(default)]
    pub level: Option<ExerciseStage>,
    /// Выбранный набор упражнений (`starter`, `daily`, …).
    #[serde(default)]
    pub pack_id: Option<String>,
    /// Локальная карта: что получается по звукам/слогам/словам/фразам.
    #[serde(default)]
    pub speech_map: SpeechMap,
    /// Хвост последних занятий (для тренда на экране прогресса).
    #[serde(default)]
    pub session_history: Vec<SessionRecord>,
}

/// Одна завершённая практика (не диагностика).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRecord {
    pub correct: u32,
    pub total: u32,
}

const SESSION_HISTORY_MAX: usize = 20;

impl Progress {
    pub fn record_session(&mut self, correct: u32, total: u32) {
        self.sessions_completed += 1;
        self.total_correct += correct;
        self.total_answered += total;
        self.session_history.push(SessionRecord { correct, total });
        if self.session_history.len() > SESSION_HISTORY_MAX {
            let drop_n = self.session_history.len() - SESSION_HISTORY_MAX;
            self.session_history.drain(..drop_n);
        }
    }

    pub fn set_level(&mut self, level: ExerciseStage) {
        self.level = Some(level);
    }

    pub fn set_pack(&mut self, pack_id: &str) {
        self.pack_id = Some(pack_id.to_string());
    }

    pub fn record_speech(&mut self, exercise: &Exercise, correct: bool) {
        if let Some(key) = exercise.map_key() {
            self.speech_map.record(&key, correct);
        }
    }

    /// Доля верных по хвосту истории (0…1), если она не пуста.
    pub fn recent_accuracy(&self) -> Option<f32> {
        if self.session_history.is_empty() {
            return None;
        }
        let (c, t) = self
            .session_history
            .iter()
            .fold((0u32, 0u32), |(c, t), s| (c + s.correct, t + s.total));
        if t == 0 {
            None
        } else {
            Some(c as f32 / t as f32)
        }
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
        assert!(speech_matches("а а а", "а"));
        assert!(speech_matches("м м м", "эм"));
        assert!(!speech_matches("м м м", "пэ"));
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
                stage: Some(ExerciseStage::Sound),
                prompt: "v".into(),
                text: "А".into(),
                speak: Some("а а а".into()),
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
                ExerciseStage::Sound,
                ExerciseStage::Syllable,
                ExerciseStage::Word,
                ExerciseStage::Phrase
            ]
        );
        assert_eq!(ordered[0].target_text(), Some("а а а"));
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
        // Меньше половины на звуках.
        assert_eq!(
            infer_level(&[
                (ExerciseStage::Sound, false),
                (ExerciseStage::Sound, false),
                (ExerciseStage::Syllable, true),
            ]),
            ExerciseStage::Sound
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
                stage: Some(ExerciseStage::Sound),
                prompt: "v".into(),
                text: "А".into(),
                speak: None,
            },
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
        assert_eq!(d.len(), 4);
        assert_eq!(d[0].stage(), ExerciseStage::Sound);
        assert_eq!(d[1].stage(), ExerciseStage::Syllable);
        assert_eq!(d[2].stage(), ExerciseStage::Word);
        assert_eq!(d[3].stage(), ExerciseStage::Phrase);
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
        assert_eq!(p.session_history.len(), 2);
        assert_eq!(p.session_history[0], SessionRecord { correct: 3, total: 5 });
        let acc = p.recent_accuracy().unwrap();
        assert!((acc - 5.0 / 9.0).abs() < 0.001);
    }

    #[test]
    fn speech_map_key_collapses_syllable_repeats() {
        assert_eq!(speech_map_key("ма ма ма"), "ма");
        assert_eq!(speech_map_key("МА"), "ма");
        assert_eq!(speech_map_key("дом дом"), "дом");
        assert_eq!(speech_map_key("Я пью воду"), "я пью воду");
        let syllable = Exercise::ReadAloud {
            stage: Some(ExerciseStage::Syllable),
            prompt: "s".into(),
            text: "МА".into(),
            speak: Some("ма ма ма".into()),
        };
        let choose = Exercise::ChooseWord {
            stage: Some(ExerciseStage::Word),
            prompt: "w".into(),
            options: vec!["ма".into(), "па".into()],
            answer: "ма".into(),
        };
        assert_eq!(syllable.map_key(), choose.map_key());
        assert_eq!(syllable.map_key().as_deref(), Some("ма"));
    }

    #[test]
    fn speech_map_normalize_merges_legacy_keys() {
        let mut map = SpeechMap::default();
        map.items.insert(
            "ма ма ма".into(),
            WordStat {
                correct: 1,
                attempts: 2,
            },
        );
        map.items.insert(
            "ма".into(),
            WordStat {
                correct: 1,
                attempts: 1,
            },
        );
        map.normalize_keys();
        let s = map.items.get("ма").unwrap();
        assert_eq!(s.correct, 2);
        assert_eq!(s.attempts, 3);
        assert!(!map.items.contains_key("ма ма ма"));
    }

    #[test]
    fn pack_speech_entries_stage_order() {
        let pack = ExercisePack {
            title: "t".into(),
            exercises: vec![
                Exercise::ReadAloud {
                    stage: Some(ExerciseStage::Phrase),
                    prompt: "p".into(),
                    text: "доброе утро".into(),
                    speak: None,
                },
                Exercise::ReadAloud {
                    stage: Some(ExerciseStage::Sound),
                    prompt: "v".into(),
                    text: "А".into(),
                    speak: Some("а а а".into()),
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
            ],
        };
        let entries = pack_speech_entries(&pack, &SpeechMap::default());
        let stages: Vec<_> = entries.iter().map(|e| e.stage).collect();
        assert_eq!(
            stages,
            vec![
                ExerciseStage::Sound,
                ExerciseStage::Syllable,
                ExerciseStage::Word,
                ExerciseStage::Phrase
            ]
        );
        assert_eq!(entries[0].label, "А");
    }

    #[test]
    fn speech_map_ratings() {
        let mut map = SpeechMap::default();
        map.record("мама", true);
        map.record("мама", true);
        map.record("мама", true);
        assert_eq!(map.rating("мама"), SpeechRating::Good);
        map.record("па", false);
        map.record("па", false);
        assert_eq!(map.rating("па"), SpeechRating::Weak);
        assert_eq!(map.rating("нет"), SpeechRating::Unknown);
    }

    #[test]
    fn smart_order_puts_weak_first_in_stage() {
        let pack = vec![
            Exercise::ReadAloud {
                stage: Some(ExerciseStage::Word),
                prompt: "a".into(),
                text: "хлеб".into(),
                speak: None,
            },
            Exercise::ReadAloud {
                stage: Some(ExerciseStage::Word),
                prompt: "b".into(),
                text: "чай".into(),
                speak: None,
            },
            Exercise::ReadAloud {
                stage: Some(ExerciseStage::Word),
                prompt: "c".into(),
                text: "стол".into(),
                speak: None,
            },
        ];
        let mut map = SpeechMap::default();
        map.record("хлеб", true);
        map.record("хлеб", true);
        map.record("чай", false);
        map.record("чай", false);
        let ordered = order_session_for_level_with_map(pack, ExerciseStage::Word, &map, true);
        let labels: Vec<_> = ordered
            .iter()
            .filter_map(|e| e.map_label())
            .collect();
        assert_eq!(labels[0], "чай");
        assert_eq!(labels.last().unwrap(), "хлеб");
    }

    #[test]
    fn twister_unlock_by_phrase_level_or_map_ratio() {
        let pack = ExercisePack {
            title: "t".into(),
            exercises: vec![
                Exercise::ReadAloud {
                    stage: Some(ExerciseStage::Phrase),
                    prompt: "p".into(),
                    text: "доброе утро".into(),
                    speak: None,
                },
                Exercise::ReadAloud {
                    stage: Some(ExerciseStage::Phrase),
                    prompt: "p2".into(),
                    text: "спокойной ночи".into(),
                    speak: None,
                },
                Exercise::ReadAloud {
                    stage: Some(ExerciseStage::Twister),
                    prompt: "t".into(),
                    text: "шла саша по шоссе".into(),
                    speak: None,
                },
            ],
        };
        let map = SpeechMap::default();
        assert!(!twister_unlocked(Some(ExerciseStage::Word), &pack, &map));
        assert!(twister_unlocked(Some(ExerciseStage::Phrase), &pack, &map));

        let mut map = SpeechMap::default();
        map.record("доброе утро", true);
        map.record("доброе утро", true);
        map.record("спокойной ночи", true);
        map.record("спокойной ночи", true);
        assert!(twister_unlocked(Some(ExerciseStage::Word), &pack, &map));

        let without = order_session_for_level_with_map(
            pack.exercises.clone(),
            ExerciseStage::Word,
            &SpeechMap::default(),
            false,
        );
        assert!(without.iter().all(|e| e.stage() != ExerciseStage::Twister));
        let with = order_session_for_level_with_map(
            pack.exercises.clone(),
            ExerciseStage::Word,
            &map,
            true,
        );
        assert!(with.iter().any(|e| e.stage() == ExerciseStage::Twister));
    }
}
