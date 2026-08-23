use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;

use super::exercise::{ExercisePack, Progress};

const PACK_BYTES: &[u8] = include_bytes!("../../assets/exercises/starter.json");

pub fn load_starter_pack() -> Result<ExercisePack, String> {
    let pack: ExercisePack = serde_json::from_slice(PACK_BYTES)
        .map_err(|e| format!("Не удалось разобрать упражнения: {e}"))?;
    validate_pack(&pack)?;
    Ok(pack)
}

fn validate_pack(pack: &ExercisePack) -> Result<(), String> {
    use super::exercise::Exercise;
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
                if super::exercise::normalize_phrase(&joined)
                    != super::exercise::normalize_phrase(answer)
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
    let dirs = ProjectDirs::from("app", "SoftEcho", "SoftEcho")
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

/// Новый файл для длинного диктофона: `dictaphone_YYYYMMDD_HHMMSS.txt`.
pub fn new_dictaphone_path() -> Result<PathBuf, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dir = data_dir()?.join("dictaphone");
    fs::create_dir_all(&dir).map_err(|e| format!("Не удалось создать {dir:?}: {e}"))?;
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Простой штамп без chrono-зависимостей.
    let name = format!("dictaphone_{secs}.txt");
    Ok(dir.join(name))
}

/// Дописать фрагмент в txt (длинная запись — сразу на диск).
pub fn append_dictaphone_text(path: &std::path::Path, chunk: &str) -> Result<(), String> {
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("Не удалось открыть {path:?}: {e}"))?;
    write!(f, "{chunk}").map_err(|e| format!("Не удалось дописать {path:?}: {e}"))?;
    Ok(())
}

/// Перезаписать txt полным текстом (кнопка «Сохранить»).
pub fn save_dictaphone_text(path: &std::path::Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Не удалось создать {parent:?}: {e}"))?;
    }
    fs::write(path, text).map_err(|e| format!("Не удалось записать {path:?}: {e}"))
}

/// Каталог модели Vosk: рядом с exe (portable), cwd, или данные пользователя.
pub fn vosk_model_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    // Portable: папка с бинарником
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("vosk-model-small-ru-0.22"));
            candidates.push(dir.join("assets/vosk/vosk-model-small-ru-0.22"));
            candidates.push(dir.join("vosk/vosk-model-small-ru-0.22"));
        }
    }

    candidates.push(PathBuf::from("assets/vosk/model"));
    candidates.push(PathBuf::from("assets/vosk/vosk-model-small-ru-0.22"));

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
        assert!(!pack.exercises.is_empty());
    }

    #[test]
    fn dictaphone_txt_append_and_save() {
        let dir = std::env::temp_dir().join(format!("softecho-dictaphone-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("dictaphone_test.txt");
        append_dictaphone_text(&path, "раз").unwrap();
        append_dictaphone_text(&path, "\nдва").unwrap();
        let got = fs::read_to_string(&path).unwrap();
        assert_eq!(got, "раз\nдва");
        save_dictaphone_text(&path, "итог").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "итог");
        let _ = fs::remove_dir_all(&dir);
    }
}
