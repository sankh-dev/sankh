//! Per-user directories: settings (trust, workspace) and data (Scratch).

use std::path::PathBuf;

/// `$SANKH_CONFIG_DIR`, else `~/.config/sankh` (platform equivalent).
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SANKH_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sankh")
}

/// `$SANKH_DATA_DIR`, else `~/.local/share/sankh` (platform equivalent).
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SANKH_DATA_DIR") {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sankh")
}
