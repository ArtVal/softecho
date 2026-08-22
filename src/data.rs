use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;

use crate::exercise::{ExercisePack, Progress};

const PACK_BYTES: &[u8] = include_bytes!("../assets/exercises/starter.json");

pub fn load_starter_pack() -> Result<ExercisePack, String> {
    let pack: ExercisePack = serde_json::from_slice(PACK_BYTES)
        .map_err(|e| format!("Не удалось разобрать упражнения: {e}"))?;
    validate_pack(&pack)?;
    Ok(pack)
}

fn validate_pack(pack: &ExercisePack) -> Result<(), String> {
    use crate::exercise::Exercise;
    if pack.exercises.is_empty() {
        return Err("Набор упражнений пуст".into());
    }
    for (i, ex) in pack.exercises.iter().enumerate() {
        match ex {
            Exercise::ChooseWord {
                options, answer, ..
            } => {
                if options.len() < 2 {
                    return Err(format!("Упражнение {i}: мало вариантов"));
                }
                if !options.iter().any(|o| o == answer) {
                    return Err(format!(
                        "Упражнение {i}: ответ «{answer}» не входит в варианты"
                    ));
                }
            }
            Exercise::BuildPhrase { words, answer, .. } => {
                if words.is_empty() {
                    return Err(format!("Упражнение {i}: нет слов"));
                }
                let joined = words.join(" ");
                if crate::exercise::normalize_phrase(&joined)
                    != crate::exercise::normalize_phrase(answer)
                {
                    return Err(format!(
                        "Упражнение {i}: слова не совпадают с ответом «{answer}»"
                    ));
                }
            }
            Exercise::ReadAloud { text, .. } => {
                if text.trim().is_empty() {
                    return Err(format!("Упражнение {i}: пустой текст"));
                }
            }
        }
    }
    Ok(())
}

fn data_dir() -> Result<PathBuf, String> {
    let dirs = ProjectDirs::from("ru", "stroke", "stroke_trainer")
        .ok_or_else(|| "Не удалось определить каталог данных".to_string())?;
    let path = dirs.data_dir().to_path_buf();
    fs::create_dir_all(&path).map_err(|e| format!("Не удалось создать {path:?}: {e}"))?;
    Ok(path)
}

fn progress_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("progress.json"))
}

pub fn load_progress() -> Progress {
    let Ok(path) = progress_path() else {
        return Progress::default();
    };
    let Ok(bytes) = fs::read(&path) else {
        return Progress::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn save_progress(progress: &Progress) -> Result<(), String> {
    let path = progress_path()?;
    let bytes = serde_json::to_vec_pretty(progress)
        .map_err(|e| format!("Не удалось сериализовать прогресс: {e}"))?;
    fs::write(&path, bytes).map_err(|e| format!("Не удалось записать {path:?}: {e}"))
}

/// Каталог для модели Vosk: рядом с бинарём или в данных пользователя.
pub fn vosk_model_dir() -> Option<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("assets/vosk/model"),
        PathBuf::from("assets/vosk/vosk-model-small-ru-0.22"),
    ];
    if let Ok(dir) = data_dir() {
        candidates.push(dir.join("vosk-model-small-ru-0.22"));
    }
    candidates.into_iter().find(|p| p.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_pack_loads_and_validates() {
        let pack = load_starter_pack().expect("starter.json должен разбираться");
        assert_eq!(pack.title, "Стартовый набор");
        assert!(pack.exercises.len() >= 3);
    }
}
