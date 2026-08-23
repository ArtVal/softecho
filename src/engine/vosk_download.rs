//! Скачивание языковой модели Vosk в каталог данных пользователя.

use std::fs::{self, File};
use std::io::{copy, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Duration;

pub const MODEL_DIR_NAME: &str = "vosk-model-small-ru-0.22";
const MODEL_URL: &str =
    "https://huggingface.co/rhasspy/vosk-models/resolve/main/ru/vosk-model-small-ru-0.22.zip";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone)]
pub enum DownloadMsg {
    Phase(String),
    Percent(u8),
    Done,
    Err(String),
}

pub fn spawn_model_download(dest_parent: PathBuf, tx: Sender<DownloadMsg>) {
    std::thread::spawn(move || {
        if let Err(e) = download_model(&dest_parent, &tx) {
            let _ = tx.send(DownloadMsg::Err(e));
        }
    });
}

fn download_model(dest_parent: &Path, tx: &Sender<DownloadMsg>) -> Result<(), String> {
    fs::create_dir_all(dest_parent).map_err(|e| format!("Не удалось создать каталог: {e}"))?;

    let tmp_zip = dest_parent.join("vosk-model-download.tmp.zip");
    let _ = fs::remove_file(&tmp_zip);

    let result = download_model_inner(dest_parent, &tmp_zip, tx);
    if result.is_err() {
        let _ = fs::remove_file(&tmp_zip);
    }
    result
}

fn download_model_inner(
    dest_parent: &Path,
    tmp_zip: &Path,
    tx: &Sender<DownloadMsg>,
) -> Result<(), String> {
    let _ = tx.send(DownloadMsg::Phase("Скачиваю модель (~45 МБ)…".into()));

    let resp = ureq::get(MODEL_URL)
        .timeout(DOWNLOAD_TIMEOUT)
        .call()
        .map_err(|e| format!("Не удалось скачать: {e}"))?;

    let total = resp
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n > 0);

    let mut reader = resp.into_reader();
    let mut file = File::create(tmp_zip).map_err(|e| format!("Не удалось записать файл: {e}"))?;
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded = 0u64;
    let mut last_percent = 255u8;

    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Ошибка чтения: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("Ошибка записи: {e}"))?;
        downloaded += n as u64;
        if let Some(total) = total {
            let p = ((downloaded.saturating_mul(100)) / total).min(99) as u8;
            if p != last_percent {
                last_percent = p;
                let _ = tx.send(DownloadMsg::Percent(p));
            }
        }
    }
    drop(file);

    let _ = tx.send(DownloadMsg::Phase("Распаковка…".into()));

    let model_path = dest_parent.join(MODEL_DIR_NAME);
    if model_path.is_dir() {
        fs::remove_dir_all(&model_path).map_err(|e| format!("Не удалось очистить каталог: {e}"))?;
    }

    extract_zip(tmp_zip, dest_parent)?;
    let _ = fs::remove_file(tmp_zip);

    if !model_path.is_dir() {
        return Err("В архиве нет папки vosk-model-small-ru-0.22".into());
    }

    let _ = tx.send(DownloadMsg::Done);
    Ok(())
}

fn extract_zip(zip_path: &Path, dest_parent: &Path) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| format!("Не открыть архив: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Повреждённый архив: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(relative) = entry.enclosed_name().map(|p| p.to_owned()) else {
            continue;
        };
        let out = dest_parent.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out_file = File::create(&out).map_err(|e| e.to_string())?;
            copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn write_test_zip(path: &Path) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.add_directory(format!("{MODEL_DIR_NAME}/"), options)
            .unwrap();
        zip.start_file(format!("{MODEL_DIR_NAME}/README"), options)
            .unwrap();
        zip.write_all(b"ok").unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn extract_zip_creates_model_dir() {
        let dir = std::env::temp_dir().join(format!("softecho-zip-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("test.zip");
        write_test_zip(&zip_path);
        extract_zip(&zip_path, &dir).unwrap();
        assert!(dir.join(MODEL_DIR_NAME).join("README").is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn extract_zip_does_not_require_model_dir_in_archive_name() {
        let dir = std::env::temp_dir().join(format!("softecho-zip-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("bad.zip");
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.start_file("other.txt", options).unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
        extract_zip(&zip_path, &dir).unwrap();
        assert!(!dir.join(MODEL_DIR_NAME).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
