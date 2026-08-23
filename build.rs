//! Линковка нативной libvosk при feature = "asr".
//! Linux: libvosk.so · Windows: libvosk.dll (+ .lib для MSVC).

fn main() {
    if std::env::var_os("CARGO_FEATURE_ASR").is_none() {
        return;
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let lib_dir = manifest.join("native/vosk");

    match target_os.as_str() {
        "linux" => setup_linux(&manifest, &lib_dir),
        "windows" => setup_windows(&manifest, &lib_dir),
        other => panic!(
            "ASR на '{other}' пока не настроен в build.rs. Нужны файлы в native/vosk/."
        ),
    }
}

fn setup_linux(manifest: &std::path::Path, lib_dir: &std::path::Path) {
    let lib = lib_dir.join("libvosk.so");
    if !lib.exists() {
        panic!(
            "Не найден {}. Скачайте vosk-linux-x86_64 с \
             https://github.com/alphacep/vosk-api/releases (v0.3.45) \
             и положите libvosk.so в native/vosk/. Или: ./scripts/setup-asr.sh",
            lib.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=vosk");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:rerun-if-changed={}", lib.display());

    copy_next_to_binary(manifest, &lib, "libvosk.so");
}

fn setup_windows(manifest: &std::path::Path, lib_dir: &std::path::Path) {
    let dll = lib_dir.join("libvosk.dll");
    if !dll.exists() {
        panic!(
            "Не найден {}. Скачайте vosk-win64 с \
             https://github.com/alphacep/vosk-api/releases (v0.3.45) \
             и положите libvosk.dll (и libvosk.lib) в native/vosk/. \
             Или: ./scripts/fetch-vosk-windows.sh",
            dll.display()
        );
    }

    // Import-lib для MSVC/gnu: libvosk.lib или vosk.lib
    let has_import = lib_dir.join("libvosk.lib").exists() || lib_dir.join("vosk.lib").exists();
    if !has_import {
        eprintln!(
            "cargo:warning=Нет libvosk.lib / vosk.lib в {} — линковка Windows может упасть",
            lib_dir.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    // Имя как у Alphacephei: libvosk.dll / libvosk.lib
    println!("cargo:rustc-link-lib=dylib=libvosk");
    println!("cargo:rerun-if-changed={}", dll.display());

    copy_next_to_binary(manifest, &dll, "libvosk.dll");
    // На случай, если загрузчик ищет vosk.dll
    copy_next_to_binary(manifest, &dll, "vosk.dll");
}

fn copy_next_to_binary(manifest: &std::path::Path, src: &std::path::Path, dest_name: &str) {
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let target = std::env::var("TARGET").unwrap_or_default();
    let host = std::env::var("HOST").unwrap_or_default();

    let mut dirs = vec![manifest.join("target").join(&profile)];
    // Кросс-сборка: target/<triple>/<profile>/
    if !target.is_empty() && target != host {
        dirs.push(manifest.join("target").join(&target).join(&profile));
    }
    // CARGO_TARGET_DIR / OUT_DIR рядом с deps
    if let Ok(out) = std::env::var("OUT_DIR") {
        let out = std::path::PathBuf::from(out);
        // .../target/<profile>/build/<crate>/out → вверх к <profile>
        if let Some(profile_dir) = out.ancestors().nth(3) {
            dirs.push(profile_dir.to_path_buf());
        }
    }

    for dir in dirs {
        let _ = std::fs::create_dir_all(&dir);
        let dest = dir.join(dest_name);
        let _ = std::fs::copy(src, &dest);
    }
}
