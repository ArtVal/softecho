//! Скачивание языковой модели Vosk в каталог данных пользователя.

use std::fs::{self, File};
use std::io::{copy, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::i18n::{tr, AppLanguage};

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone)]
pub enum DownloadMsg {
    Phase(String),
    Percent(u8),
    Done,
    Err(String),
}

pub fn spawn_model_download(
    dest_parent: PathBuf,
    language: AppLanguage,
    tx: Sender<DownloadMsg>,
    cancel: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        if let Err(e) = download_model(&dest_parent, language, &tx, &cancel) {
            let _ = tx.send(DownloadMsg::Err(e));
        }
    })
}

fn download_model(
    dest_parent: &Path,
    language: AppLanguage,
    tx: &Sender<DownloadMsg>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err(tr(language, "err_download_cancel").into());
    }
    fs::create_dir_all(dest_parent).map_err(|e| {
        format!("{}: {e}", tr(language, "err_mkdir"))
    })?;

    // Уникальное имя: смена языка / повторный старт не дерутся за один tmp.
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let tmp_zip = dest_parent.join(format!(
        "vosk-model-download-{}-{stamp}.tmp.zip",
        language.vosk_model_dir_name()
    ));
    let _ = fs::remove_file(&tmp_zip);

    let result = download_model_inner(dest_parent, language, &tmp_zip, tx, cancel);
    let _ = fs::remove_file(&tmp_zip);
    result
}

fn download_model_inner(
    dest_parent: &Path,
    language: AppLanguage,
    tmp_zip: &Path,
    tx: &Sender<DownloadMsg>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let size = language.vosk_model_size_hint();
    let _ = tx.send(DownloadMsg::Phase(format!(
        "{} ({size})…",
        tr(language, "download_fetching")
    )));

    let resp = ureq::get(language.vosk_model_url())
        .timeout(DOWNLOAD_TIMEOUT)
        .call()
        .map_err(|e| format!("{}: {e}", tr(language, "err_download")))?;

    let total = resp
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n > 0);

    let mut reader = resp.into_reader();
    let mut file = File::create(tmp_zip)
        .map_err(|e| format!("{}: {e}", tr(language, "err_write_file")))?;
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded = 0u64;
    let mut last_percent = 255u8;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(tr(language, "err_download_cancel").into());
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", tr(language, "err_read")))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("{}: {e}", tr(language, "err_write")))?;
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

    if cancel.load(Ordering::Relaxed) {
        return Err(tr(language, "err_download_cancel").into());
    }

    let _ = tx.send(DownloadMsg::Phase(tr(language, "download_unpack").into()));
    if cancel.load(Ordering::Relaxed) {
        return Err(tr(language, "err_download_cancel").into());
    }

    extract_zip(tmp_zip, language, dest_parent, cancel)?;

    if cancel.load(Ordering::Relaxed) {
        return Err(tr(language, "err_download_cancel").into());
    }

    let model_name = language.vosk_model_dir_name();
    let model_path = dest_parent.join(model_name);
    if !model_path.is_dir() {
        return Err(format!("{} {model_name}", tr(language, "err_zip_missing")));
    }

    let _ = tx.send(DownloadMsg::Done);
    Ok(())
}

fn extract_zip(
    zip_path: &Path,
    language: AppLanguage,
    dest_parent: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let model_name = language.vosk_model_dir_name();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let staging = dest_parent.join(format!("{model_name}.staging-{stamp}"));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;

    let file = File::open(zip_path)
        .map_err(|e| format!("{}: {e}", tr(language, "err_zip_open")))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("{}: {e}", tr(language, "err_zip_bad")))?;

    let mut wrote_any = false;
    for i in 0..archive.len() {
        if cancel.load(Ordering::Relaxed) {
            let _ = fs::remove_dir_all(&staging);
            return Err(tr(language, "err_download_cancel").into());
        }
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(relative) = entry.enclosed_name().map(|p| p.to_owned()) else {
            continue;
        };
        let mut comps = relative.components();
        let Some(std::path::Component::Normal(first)) = comps.next() else {
            continue;
        };
        // Только файлы внутри каталога модели — иначе zip мог бы затереть progress/packs.
        if first != std::ffi::OsStr::new(model_name) {
            continue;
        }
        let rest: PathBuf = comps.collect();
        if rest.as_os_str().is_empty() {
            if entry.is_dir() {
                continue;
            }
            // файл прямо как model_name — не ожидаем
            continue;
        }
        let out = staging.join(&rest);
        if !out.starts_with(&staging) {
            continue;
        }
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out_file = File::create(&out).map_err(|e| e.to_string())?;
            copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;
            wrote_any = true;
        }
    }

    if cancel.load(Ordering::Relaxed) {
        let _ = fs::remove_dir_all(&staging);
        return Err(tr(language, "err_download_cancel").into());
    }

    if !wrote_any {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("{} {model_name}", tr(language, "err_zip_missing")));
    }

    let final_path = dest_parent.join(model_name);
    let backup = dest_parent.join(format!("{model_name}.old-{stamp}"));
    if final_path.is_dir() {
        fs::rename(&final_path, &backup).map_err(|e| {
            let _ = fs::remove_dir_all(&staging);
            format!("{}: {e}", tr(language, "err_clear_dir"))
        })?;
    }
    if cancel.load(Ordering::Relaxed) {
        let _ = fs::remove_dir_all(&staging);
        if backup.is_dir() {
            let _ = fs::rename(&backup, &final_path);
        }
        return Err(tr(language, "err_download_cancel").into());
    }
    if let Err(e) = fs::rename(&staging, &final_path) {
        let _ = fs::remove_dir_all(&staging);
        if backup.is_dir() {
            let _ = fs::rename(&backup, &final_path);
        }
        return Err(format!("{}: {e}", tr(language, "err_write")));
    }
    let _ = fs::remove_dir_all(&backup);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn write_test_zip(path: &Path, model_name: &str) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.add_directory(format!("{model_name}/"), options)
            .unwrap();
        zip.start_file(format!("{model_name}/README"), options)
            .unwrap();
        zip.write_all(b"ok").unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn extract_zip_creates_model_dir() {
        let model_name = AppLanguage::Ru.vosk_model_dir_name();
        let dir = std::env::temp_dir().join(format!("softecho-zip-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("test.zip");
        write_test_zip(&zip_path, model_name);
        let cancel = AtomicBool::new(false);
        extract_zip(&zip_path, AppLanguage::Ru, &dir, &cancel).unwrap();
        assert!(dir.join(model_name).join("README").is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn extract_zip_rejects_entries_outside_model_dir() {
        let model_name = AppLanguage::En.vosk_model_dir_name();
        let dir = std::env::temp_dir().join(format!("softecho-zip-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("bad.zip");
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.start_file("other.txt", options).unwrap();
        zip.write_all(b"x").unwrap();
        zip.start_file("progress.json", options).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.finish().unwrap();
        let cancel = AtomicBool::new(false);
        assert!(extract_zip(&zip_path, AppLanguage::En, &dir, &cancel).is_err());
        assert!(!dir.join(model_name).exists());
        assert!(!dir.join("other.txt").exists());
        assert!(!dir.join("progress.json").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn extract_zip_stops_on_cancel_before_promote() {
        let model_name = AppLanguage::Ru.vosk_model_dir_name();
        let dir = std::env::temp_dir().join(format!(
            "softecho-zip-cancel-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("test.zip");
        write_test_zip(&zip_path, model_name);
        let cancel = AtomicBool::new(true);
        let err = extract_zip(&zip_path, AppLanguage::Ru, &dir, &cancel).unwrap_err();
        assert!(err.contains("отменен") || err.contains("cancelled"));
        assert!(!dir.join(model_name).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
