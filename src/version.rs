//! Версия приложения (задаётся в `build.rs` → `SOFTECHO_VERSION`).

/// Semver из Cargo.toml; для сборок не с тега может быть суффикс ` · git-describe`.
pub const APP_VERSION: &str = env!("SOFTECHO_VERSION");
