use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;

use super::exercise::{Exercise, ExercisePack, Progress};
use super::i18n::AppLanguage;

/// Пользовательский файл набора: активные + отключённые.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EditablePack {
    pub title: String,
    pub exercises: Vec<Exercise>,
    #[serde(default)]
    pub disabled: Vec<Exercise>,
}

impl EditablePack {
    pub fn from_pack(pack: ExercisePack) -> Self {
        Self {
            title: pack.title,
            exercises: pack.exercises,
            disabled: Vec::new(),
        }
    }

    pub fn to_active_pack(&self) -> ExercisePack {
        ExercisePack {
            title: self.title.clone(),
            exercises: self.exercises.clone(),
        }
    }
}

pub const DEFAULT_PACK_ID: &str = "starter";

struct EmbeddedPack {
    id: &'static str,
    language: AppLanguage,
    bytes: &'static [u8],
}

const EMBEDDED_PACKS: &[EmbeddedPack] = &[
    EmbeddedPack {
        id: "starter",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/starter.json"),
    },
    EmbeddedPack {
        id: "starter_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/starter_en.json"),
    },
    EmbeddedPack {
        id: "sounds",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/sounds.json"),
    },
    EmbeddedPack {
        id: "sounds_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/sounds_en.json"),
    },
    EmbeddedPack {
        id: "syllables",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/syllables.json"),
    },
    EmbeddedPack {
        id: "syllables_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/syllables_en.json"),
    },
    EmbeddedPack {
        id: "odk",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/odk.json"),
    },
    EmbeddedPack {
        id: "odk_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/odk_en.json"),
    },
    EmbeddedPack {
        id: "rhymes",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/rhymes.json"),
    },
    EmbeddedPack {
        id: "rhymes_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/rhymes_en.json"),
    },
    EmbeddedPack {
        id: "twisters",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/twisters.json"),
    },
    EmbeddedPack {
        id: "twisters_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/twisters_en.json"),
    },
    EmbeddedPack {
        id: "daily",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/daily.json"),
    },
    EmbeddedPack {
        id: "daily_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/daily_en.json"),
    },
    EmbeddedPack {
        id: "greetings",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/greetings.json"),
    },
    EmbeddedPack {
        id: "greetings_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/greetings_en.json"),
    },
    EmbeddedPack {
        id: "family",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/family.json"),
    },
    EmbeddedPack {
        id: "family_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/family_en.json"),
    },
    EmbeddedPack {
        id: "body",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/body.json"),
    },
    EmbeddedPack {
        id: "body_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/body_en.json"),
    },
    EmbeddedPack {
        id: "food",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/food.json"),
    },
    EmbeddedPack {
        id: "food_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/food_en.json"),
    },
    EmbeddedPack {
        id: "transport",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/transport.json"),
    },
    EmbeddedPack {
        id: "transport_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/transport_en.json"),
    },
    EmbeddedPack {
        id: "pictures",
        language: AppLanguage::Ru,
        bytes: include_bytes!("../../assets/exercises/pictures.json"),
    },
    EmbeddedPack {
        id: "pictures_en",
        language: AppLanguage::En,
        bytes: include_bytes!("../../assets/exercises/pictures_en.json"),
    },
];

/// Язык встроенного набора; пользовательские — без привязки (видны всегда).
pub fn builtin_pack_language(id: &str) -> Option<AppLanguage> {
    EMBEDDED_PACKS
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.language)
}

pub fn pack_matches_language(id: &str, language: AppLanguage) -> bool {
    match builtin_pack_language(id) {
        Some(pack_lang) => pack_lang == language,
        None => true,
    }
}

/// Краткое описание набора (для экрана выбора).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackCatalogEntry {
    pub id: String,
    pub title: String,
    /// Можно править в редакторе (пользовательский файл).
    pub editable: bool,
}

pub fn list_builtin_packs_for(language: Option<AppLanguage>) -> Vec<PackCatalogEntry> {
    EMBEDDED_PACKS
        .iter()
        .filter(|entry| match language {
            Some(lang) => entry.language == lang,
            None => true,
        })
        .filter_map(|entry| {
            load_pack_bytes(entry.id, entry.bytes)
                .ok()
                .map(|pack| PackCatalogEntry {
                    id: entry.id.to_string(),
                    title: pack.title,
                    editable: false,
                })
        })
        .collect()
}

pub fn list_user_packs() -> Vec<PackCatalogEntry> {
    let Ok(dir) = packs_dir() else {
        return Vec::new();
    };
    let Ok(rd) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for ent in rd.flatten() {
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Ok(pack) = load_pack_file(stem, &path) {
            out.push(PackCatalogEntry {
                id: stem.to_string(),
                title: pack.title,
                editable: true,
            });
        }
    }
    out.sort_by(|a, b| a.title.cmp(&b.title));
    out
}

pub fn list_packs_for(language: Option<AppLanguage>) -> Vec<PackCatalogEntry> {
    let mut out = list_builtin_packs_for(language);
    out.extend(list_user_packs());
    out
}

pub fn is_user_pack(id: &str) -> bool {
    packs_dir()
        .ok()
        .map(|d| d.join(format!("{id}.json")).is_file())
        .unwrap_or(false)
}

pub fn load_pack(id: &str) -> Result<ExercisePack, String> {
    if let Ok(dir) = packs_dir() {
        let path = dir.join(format!("{id}.json"));
        if path.is_file() {
            return load_pack_file(id, &path);
        }
    }
    let Some(entry) = EMBEDDED_PACKS.iter().find(|p| p.id == id) else {
        return Err(format!("Неизвестный набор «{id}»"));
    };
    load_pack_bytes(entry.id, entry.bytes)
}

fn load_pack_bytes(id: &str, bytes: &[u8]) -> Result<ExercisePack, String> {
    // Builtin / user: лишнее поле `disabled` игнорируем через EditablePack.
    let editable: EditablePack = serde_json::from_slice(bytes)
        .map_err(|e| format!("Не удалось разобрать набор «{id}»: {e}"))?;
    let pack = editable.to_active_pack();
    validate_pack(&pack)?;
    Ok(pack)
}

fn load_pack_file(id: &str, path: &std::path::Path) -> Result<ExercisePack, String> {
    let bytes = fs::read(path).map_err(|e| format!("Не удалось прочитать {path:?}: {e}"))?;
    load_pack_bytes(id, &bytes)
}

pub fn load_editable_pack(id: &str) -> Result<EditablePack, String> {
    if let Ok(dir) = packs_dir() {
        let path = dir.join(format!("{id}.json"));
        if path.is_file() {
            let bytes =
                fs::read(&path).map_err(|e| format!("Не удалось прочитать {path:?}: {e}"))?;
            let editable: EditablePack = serde_json::from_slice(&bytes)
                .map_err(|e| format!("Не удалось разобрать набор «{id}»: {e}"))?;
            validate_editable(&editable)?;
            return Ok(editable);
        }
    }
    let pack = load_pack(id)?;
    Ok(EditablePack::from_pack(pack))
}

pub fn save_user_pack(id: &str, editable: &EditablePack) -> Result<PathBuf, String> {
    let id = sanitize_pack_id(id)?;
    validate_editable(editable)?;
    let dir = packs_dir()?;
    let path = dir.join(format!("{id}.json"));
    let bytes = serde_json::to_vec_pretty(editable)
        .map_err(|e| format!("Не удалось сериализовать набор: {e}"))?;
    fs::write(&path, bytes).map_err(|e| format!("Не удалось записать {path:?}: {e}"))?;
    Ok(path)
}

/// Копия встроенного или текущего набора в каталог пользователя.
pub fn clone_pack_to_user(source_id: &str, title: &str) -> Result<(String, EditablePack), String> {
    let mut editable = load_editable_pack(source_id)?;
    if !title.trim().is_empty() {
        editable.title = title.trim().to_string();
    } else if !editable.title.contains("(мой)") {
        editable.title = format!("{} (мой)", editable.title);
    }
    let id = unique_user_pack_id(source_id)?;
    save_user_pack(&id, &editable)?;
    Ok((id, editable))
}

fn unique_user_pack_id(source_id: &str) -> Result<String, String> {
    let base = sanitize_pack_id(&format!("my-{source_id}"))?;
    if !is_user_pack(&base) && EMBEDDED_PACKS.iter().all(|p| p.id != base) {
        return Ok(base);
    }
    for n in 2..1000 {
        let id = format!("{base}-{n}");
        if !is_user_pack(&id) {
            return Ok(id);
        }
    }
    Err("Слишком много копий набора".into())
}

pub fn sanitize_pack_id(raw: &str) -> Result<String, String> {
    let s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        return Err("Пустой идентификатор набора".into());
    }
    if s.len() > 64 {
        return Err("Слишком длинный идентификатор набора".into());
    }
    Ok(s)
}

pub fn packs_dir() -> Result<PathBuf, String> {
    let dir = data_dir()?.join("packs");
    fs::create_dir_all(&dir).map_err(|e| format!("Не удалось создать {dir:?}: {e}"))?;
    Ok(dir)
}

pub fn load_active_pack(progress: &Progress) -> Result<ExercisePack, String> {
    let fallback = progress.language.default_pack_id();
    let id = progress.pack_id.as_deref().unwrap_or(fallback);
    let id = if pack_matches_language(id, progress.language) {
        id
    } else {
        fallback
    };
    load_pack(id)
}

fn validate_pack(pack: &ExercisePack) -> Result<(), String> {
    if pack.exercises.is_empty() {
        return Err("Набор упражнений пуст — включите хотя бы одно задание".into());
    }
    for (i, ex) in pack.exercises.iter().enumerate() {
        validate_one(i, ex, "активное")?;
    }
    Ok(())
}

fn validate_editable(pack: &EditablePack) -> Result<(), String> {
    validate_pack(&pack.to_active_pack())?;
    for (i, ex) in pack.disabled.iter().enumerate() {
        validate_one(i, ex, "отключённое")?;
    }
    Ok(())
}

fn validate_one(i: usize, ex: &Exercise, kind: &str) -> Result<(), String> {
    match ex {
        Exercise::ChooseWord {
            options, answer, ..
        } => {
            if options.len() < 2 {
                return Err(format!("Упражнение {kind} {i}: мало вариантов"));
            }
            if !options.iter().any(|o| o == answer) {
                return Err(format!(
                    "Упражнение {kind} {i}: ответ «{answer}» не входит в варианты"
                ));
            }
        }
        Exercise::BuildPhrase { words, answer, .. } => {
            if words.is_empty() {
                return Err(format!("Упражнение {kind} {i}: нет слов"));
            }
            let joined = words.join(" ");
            if super::exercise::normalize_phrase(&joined)
                != super::exercise::normalize_phrase(answer)
            {
                return Err(format!(
                    "Упражнение {kind} {i}: слова не совпадают с ответом «{answer}»"
                ));
            }
        }
        Exercise::ReadAloud { text, .. } => {
            if text.trim().is_empty() {
                return Err(format!("Упражнение {kind} {i}: пустой текст"));
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
    atomic_write(&path, &bytes)
}

/// Запись через `.tmp` + rename, чтобы краш не оставлял обрезанный JSON.
fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("Не удалось записать {tmp:?}: {e}"))?;
    // На Windows rename не перезаписывает существующий файл.
    if path.exists() {
        fs::remove_file(path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("Не удалось заменить {path:?}: {e}")
        })?;
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Не удалось сохранить {path:?}: {e}")
    })
}

/// Новый файл отчёта: `reports/softecho-report_YYYY….txt`.
pub fn new_report_path() -> Result<PathBuf, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dir = data_dir()?.join("reports");
    fs::create_dir_all(&dir).map_err(|e| format!("Не удалось создать {dir:?}: {e}"))?;
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(dir.join(format!("softecho-report_{secs}.txt")))
}

pub fn save_report_text(path: &std::path::Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Не удалось создать {parent:?}: {e}"))?;
    }
    fs::write(path, text).map_err(|e| format!("Не удалось записать {path:?}: {e}"))
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
pub fn vosk_model_dir(language: AppLanguage) -> Option<PathBuf> {
    let name = language.vosk_model_dir_name();
    let mut candidates = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(name));
            candidates.push(dir.join(format!("assets/vosk/{name}")));
            candidates.push(dir.join(format!("vosk/{name}")));
        }
    }

    if language == AppLanguage::Ru {
        candidates.push(PathBuf::from("assets/vosk/model"));
    }
    candidates.push(PathBuf::from(format!("assets/vosk/{name}")));

    if let Ok(dir) = data_dir() {
        candidates.push(dir.join(name));
    }

    candidates.into_iter().find(|p| p.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::exercise::ExerciseStage;

    #[test]
    fn all_builtin_packs_load() {
        for entry in list_packs_for(None) {
            let pack = load_pack(&entry.id).expect("набор должен разбираться");
            assert_eq!(pack.title, entry.title);
            assert!(!pack.exercises.is_empty());
        }
        assert!(list_packs_for(None).len() >= 26);
        assert!(
            list_packs_for(Some(AppLanguage::Ru))
                .iter()
                .any(|p| p.id == "pictures")
        );
        let pics = load_pack("pictures").expect("pictures");
        assert!(pics.exercises.iter().any(|e| e.image_id().is_some()));
        let en = list_packs_for(Some(AppLanguage::En));
        assert!(en.iter().any(|e| e.id == "starter_en"));
        assert!(en.iter().any(|e| e.id == "sounds_en"));
        assert!(en.iter().any(|e| e.id == "daily_en"));
        assert!(en.iter().any(|e| e.id == "twisters_en"));
        assert!(en.iter().any(|e| e.id == "body_en"));
        assert!(en.iter().any(|e| e.id == "transport_en"));
        assert!(en.iter().any(|e| e.id == "rhymes_en"));
        assert!(en.len() >= 13);
        assert!(!en.iter().any(|e| e.id == "daily"));
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

    #[test]
    fn atomic_write_roundtrip_and_overwrite() {
        let dir = std::env::temp_dir().join(format!("softecho-atomic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("progress.json");
        atomic_write(&path, b"{\"a\":1}").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"a\":1}");
        atomic_write(&path, b"{\"a\":2}").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"a\":2}");
        assert!(!path.with_extension("json.tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sanitize_pack_id_basic() {
        assert_eq!(sanitize_pack_id("My Pack!").unwrap(), "my-pack");
        assert!(sanitize_pack_id("???").is_err());
    }

    #[test]
    fn editable_pack_roundtrip_keeps_disabled() {
        use super::super::exercise::Exercise;
        let editable = EditablePack {
            title: "t".into(),
            exercises: vec![Exercise::ReadAloud {
                stage: Some(ExerciseStage::Word),
                prompt: "p".into(),
                text: "мама".into(),
                speak: None,
                image: None,
            }],
            disabled: vec![Exercise::ReadAloud {
                stage: Some(ExerciseStage::Word),
                prompt: "p".into(),
                text: "папа".into(),
                speak: None,
                image: None,
            }],
        };
        let bytes = serde_json::to_vec(&editable).unwrap();
        let back: EditablePack = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.disabled.len(), 1);
        assert_eq!(back.to_active_pack().exercises.len(), 1);
    }
}
