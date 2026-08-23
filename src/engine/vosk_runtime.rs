//! Windows: libvosk.dll собран MinGW и тянет runtime-DLL из папки с exe.
//! На чистой машине MinGW нет — готовим каталог до первой загрузки Vosk.

#[cfg(all(windows, feature = "asr"))]
use std::path::Path;

#[cfg(all(windows, feature = "asr"))]
pub fn prepare() {
    use std::env;

    let Ok(exe) = env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    set_dll_directory(dir);
    prepend_path(dir);
}

#[cfg(all(windows, feature = "asr"))]
fn set_dll_directory(dir: &Path) {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetDllDirectoryW(path: *const u16) -> i32;
    }

    let wide: Vec<u16> = dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        SetDllDirectoryW(wide.as_ptr());
    }
}

#[cfg(all(windows, feature = "asr"))]
fn prepend_path(dir: &Path) {
    let dir = dir.to_string_lossy();
    match std::env::var("PATH") {
        Ok(path) if path.starts_with(dir.as_ref()) => {}
        Ok(path) => {
            let _ = std::env::set_var("PATH", format!("{dir};{path}"));
        }
        Err(_) => {
            let _ = std::env::set_var("PATH", dir.to_string());
        }
    }
}

#[cfg(not(all(windows, feature = "asr")))]
pub fn prepare() {}
