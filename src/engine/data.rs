use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;

use super::exercise::{ExercisePack, Progress};

pub const DEFAULT_PACK_ID: &str = "starter";

struct EmbeddedPack {
    id: &'static str,
    bytes: &'static [u8],
}

const EMBEDDED_PACKS: &[EmbeddedPack] = &[
    EmbeddedPack {
        id: "starter",
        bytes: include_bytes!("../../assets/exercises/starter.json"),
    },
    EmbeddedPack {
        id: "sounds",
        bytes: include_bytes!("../../assets/exercises/sounds.json"),
    },
    EmbeddedPack {
        id: "syllables",
        bytes: include_bytes!("../../assets/exercises/syllables.json"),
    },
    EmbeddedPack {
        id: "odk",
        bytes: include_bytes!("../../assets/exercises/odk.json"),
    },
    EmbeddedPack {
        id: "rhymes",
        bytes: include_bytes!("../../assets/exercises/rhymes.json"),
    },
    EmbeddedPack {
        id: "twisters",
        bytes: include_bytes!("../../assets/exercises/twisters.json"),
    },
    EmbeddedPack {
        id: "daily",
        bytes: include_bytes!("../../assets/exercises/daily.json"),
    },
    EmbeddedPack {
        id: "greetings",
        bytes: include_bytes!("../../assets/exercises/greetings.json"),
    },
    EmbeddedPack {
        id: "family",
        bytes: include_bytes!("../../assets/exercises/family.json"),
    },
    EmbeddedPack {
        id: "body",
        bytes: include_bytes!("../../assets/exercises/body.json"),
    },
    EmbeddedPack {
        id: "food",
        bytes: include_bytes!("../../assets/exercises/food.json"),
    },
    EmbeddedPack {
        id: "transport",
        bytes: include_bytes!("../../assets/exercises/transport.json"),
    },
];

/// Краткое описание встроенного набора (для экрана выбора).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackCatalogEntry {
    pub id: String,
    pub title: String,
}

pub fn list_builtin_packs() -> Vec<PackCatalogEntry> {
    EMBEDDED_PACKS
        .iter()
        .filter_map(|entry| {
            load_pack_bytes(entry.id, entry.bytes)
                .ok()
                .map(|pack| PackCatalogEntry {
                    id: entry.id.to_string(),
                    title: pack.title,
                })
        })
        .collect()
}

pub fn load_pack(id: &str) -> Result<ExercisePack, String> {
    let Some(entry) = EMBEDDED_PACKS.iter().find(|p| p.id == id) else {
        return Err(format!("Неизвестный набор «{id}»"));
    };
    load_pack_bytes(entry.id, entry.bytes)
}

fn load_pack_bytes(id: &str, bytes: &[u8]) -> Result<ExercisePack, String> {
    let pack: ExercisePack = serde_json::from_slice(bytes)
        .map_err(|e| format!("Не удалось разобрать набор «{id}»: {e}"))?;
    validate_pack(&pack)?;
    Ok(pack)
}

pub fn load_active_pack(progress: &Progress) -> Result<ExercisePack, String> {
    let id = progress
        .pack_id
        .as_deref()
        .unwrap_or(DEFAULT_PACK_ID);
    load_pack(id)
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

/// Каталог данных приложения (прогресс, диктофон, модель Vosk).
pub fn user_data_dir() -> Result<PathBuf, String> {
    data_dir()
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
    let mut progress: Progress = serde_json::from_slice(&bytes).unwrap_or_default();
    progress.speech_map.normalize_keys();
    progress
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
    use super::super::exercise::ExerciseStage;

    #[test]
    fn all_builtin_packs_load() {
        for entry in list_builtin_packs() {
            let pack = load_pack(&entry.id).expect("набор должен разбираться");
            assert_eq!(pack.title, entry.title);
            assert!(!pack.exercises.is_empty());
        }
        assert!(list_builtin_packs().len() >= 12);
    }

    #[test]
    fn starter_pack_loads_and_validates() {
        let pack = load_pack(DEFAULT_PACK_ID).expect("starter.json должен разбираться");
        assert_eq!(pack.title, "Звуки → слоги → слова → фразы");
        assert!(pack
            .exercises
            .iter()
            .any(|e| e.stage() == ExerciseStage::Sound));
        assert!(pack
            .exercises
            .iter()
            .any(|e| e.stage() == ExerciseStage::Syllable));
        assert!(pack
            .exercises
            .iter()
            .any(|e| e.stage() == ExerciseStage::Word));
        assert!(pack
            .exercises
            .iter()
            .any(|e| e.stage() == ExerciseStage::Phrase));
    }

    #[test]
    fn load_active_pack_uses_progress_id() {
        let mut p = Progress::default();
        p.pack_id = Some("greetings".into());
        let pack = load_active_pack(&p).unwrap();
        assert_eq!(pack.title, "Приветствия");
    }

    #[test]
    fn sounds_pack_starts_with_vowels() {
        let pack = load_pack("sounds").expect("sounds.json должен разбираться");
        assert_eq!(pack.title, "Гласные и согласные");
        assert!(pack
            .exercises
            .iter()
            .any(|e| e.stage() == ExerciseStage::Sound));
        assert_eq!(
            pack.exercises
                .iter()
                .find(|e| e.stage() == ExerciseStage::Sound)
                .and_then(|e| e.map_label())
                .as_deref(),
            Some("А")
        );
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
