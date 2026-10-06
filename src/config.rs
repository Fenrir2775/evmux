use anyhow::Result;
use std::path::PathBuf;

pub(super) mod device_config;
pub(super) mod device_handle;
pub(super) mod macro_compiler;
pub(super) mod macros;
pub(super) mod profile;
pub(super) mod rule;
mod serde;
pub(super) mod watcher;

const CONFIG_DIR: &str = "EVMUX_CONFIG_DIR";

/// evmux root config directory.
///
/// `$HOME/.config/evmux`
fn config_dir() -> PathBuf {
    std::env::var(CONFIG_DIR)
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from(".config"))
        }).join("evmux")
}

/// Configuration directory for every device.
pub(super) fn devices_dir() -> PathBuf {
    config_dir().join("devices")
}

/// Directory to all macro files.
pub(super) fn macros_dir() -> PathBuf {
    config_dir().join("macros")
}

pub(super) fn ensure_config_dir() -> Result<()> {
    std::fs::create_dir_all(devices_dir())?;
    std::fs::create_dir_all(macros_dir())?;
    Ok(())
}
