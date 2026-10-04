use crate::config;
use crate::config::device_config::DeviceConfig;
use crate::config::macros::Macros;
use crate::config::profile::Profile;
use crate::device::input_device::InputDevice;
use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub(crate) struct DeviceHandle {
    dir: PathBuf,
    macros: Arc<RwLock<Macros>>,
}

impl DeviceHandle {
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn profile_path(&self, name: &str) -> Result<PathBuf> {
        validate_profile_name(name)?;

        Ok(self.dir.join(format!("{name}.toml")))
    }

    pub(crate) fn load(
        device: &InputDevice,
        macros: Arc<RwLock<Macros>>,
    ) -> Result<(Self, DeviceConfig, Vec<Profile>)> {
        Self::load_in(&config::devices_dir(), device, macros)
    }

    fn load_in(
        root: &Path,
        device: &InputDevice,
        macros: Arc<RwLock<Macros>>,
    ) -> Result<(Self, DeviceConfig, Vec<Profile>)> {
        let dir = find_config_dir(root, device)?.unwrap_or_else(|| generate_dir_name(root, device));
        let handle = Self { dir, macros };
        let config = load_or_init_device_config(device, &handle.dir)?;
        let profiles = handle.load_profiles()?;

        Ok((handle, config, profiles))
    }

    #[cfg(test)]
    pub(crate) fn load_for_test(
        root: &Path,
        device: &InputDevice,
        macros: Arc<RwLock<Macros>>,
    ) -> Result<(Self, DeviceConfig, Vec<Profile>)> {
        Self::load_in(root, device, macros)
    }

    pub(crate) fn reload_device_config(
        &self,
        device: &InputDevice,
    ) -> Result<(DeviceConfig, Vec<Profile>)> {
        let config = load_or_init_device_config(device, &self.dir)?;
        let profiles = self.load_profiles()?;

        Ok((config, profiles))
    }

    pub(crate) fn delete_profile(&self, profile: &str) -> Result<()> {
        std::fs::remove_file(self.profile_path(profile)?)?;

        Ok(())
    }

    /// Loads every `*.toml` profile next to `device.toml` in `dir`.
    fn load_profiles(&self) -> Result<Vec<Profile>> {
        // poisoning should be impossible
        let macros = self.macros.read().unwrap();
        let mut profiles = vec![];

        for entry in std::fs::read_dir(&self.dir)? {
            let path = entry?.path();
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };

            if path.extension().and_then(|e| e.to_str()) != Some("toml") || stem == "device" {
                continue;
            }

            let text = std::fs::read_to_string(&path)?;
            let profile = Profile::parse(stem, &text, &macros)?;

            profiles.push(profile);
        }

        Ok(profiles)
    }
}

fn find_config_dir(root: &Path, device: &InputDevice) -> Result<Option<PathBuf>> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    for entry in entries {
        let dir = entry?.path();
        if !dir.is_dir() {
            continue;
        }

        let Ok(text) = std::fs::read_to_string(dir.join("device.toml")) else {
            continue;
        };

        let Ok(existing) = toml::from_str::<DeviceConfig>(&text) else {
            continue;
        };

        if existing.device.vendor_id() == device.vendor_id()
            && existing.device.product_id() == device.product_id()
        {
            return Ok(Some(dir));
        }
    }

    Ok(None)
}

fn load_or_init_device_config(device: &InputDevice, dir: &Path) -> Result<DeviceConfig> {
    let path = dir.join("device.toml");

    if !path.exists() {
        let config = DeviceConfig {
            device: device.clone(),
            default_profile: None,
        };

        std::fs::create_dir_all(dir)?;
        std::fs::write(&path, toml::to_string_pretty(&config)?)?;
        return Ok(config);
    }

    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow!("failed to read device config {path:?}: {e:#}"))?;

    let mut config: DeviceConfig = toml::from_str(&text)
        .map_err(|e| anyhow!("failed to parse device config {path:?}: {e:#}"))?;

    config.device = device.clone();

    Ok(config)
}

fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name == "device"
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    {
        anyhow::bail!(
            "Invalid profile name: {name}: use only ASCII letters, digits, '-' and '_' and do not use 'device'"
        );
    }

    Ok(())
}

fn generate_dir_name(root_dir: &Path, device: &InputDevice) -> PathBuf {
    let base = format!(
        "{}_{:04x}_{:04x}",
        normalize(device.name()),
        device.vendor_id(),
        device.product_id()
    );

    root_dir.join(&base)
}

/// Normalizes a device name to lowercase ASCII without repeated,
/// leading or trailing separators.
fn normalize(name: &str) -> String {
    let mut ascii = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();

    while ascii.contains("__") {
        ascii = ascii.replace("__", "_");
    }

    let normalized = ascii.trim_matches('_');

    if normalized.is_empty() {
        "device".to_owned()
    } else {
        normalized.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(vendor_id: u16, product_id: u16, name: &str) -> InputDevice {
        InputDevice::new_for_test(vendor_id, product_id, name)
    }

    fn test_macros() -> Arc<RwLock<Macros>> {
        Arc::new(RwLock::new(Macros::default()))
    }

    fn write_profile(handle: &DeviceHandle, name: &str, content: &str) -> PathBuf {
        let path = handle.profile_path(name).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn name_replace_unsafe_chars_with_underscores() {
        assert_eq!(normalize("Razer Naga Trinity"), "razer_naga_trinity")
    }

    #[test]
    fn name_falls_back_if_empty_or_unsafe() {
        assert_eq!(normalize(""), "device");
        assert_eq!(normalize("///"), "device")
    }

    #[test]
    fn profile_name_accepts_alphanumeric_dash_underscore() {
        assert!(validate_profile_name("default").is_ok());
        assert!(validate_profile_name("Gaming_Profile-2").is_ok())
    }

    #[test]
    fn profile_name_rejects_empty() {
        assert!(validate_profile_name("").is_err())
    }

    #[test]
    fn profile_name_rejects_reserved() {
        assert!(validate_profile_name("device").is_err())
    }

    #[test]
    fn profile_name_rejects_path_separators() {
        assert!(validate_profile_name("../dev").is_err());
        assert!(validate_profile_name("a/b").is_err())
    }

    #[test]
    fn creates_dir_and_toml_on_first_run() {
        let tmp = tempfile::tempdir().unwrap();
        let dev = device(0x5426, 0x103, "Razer Naga Trinity");

        let (handle, config, profiles) =
            DeviceHandle::load_in(tmp.path(), &dev, test_macros()).unwrap();

        assert!(handle.dir().join("device.toml").exists());
        assert_eq!(config.default_profile, None);
        assert!(profiles.is_empty());

        let dir_name = handle.dir().file_name().unwrap().to_str().unwrap();
        assert!(dir_name.contains("5426"));
        assert!(dir_name.contains("103"));
        assert!(dir_name.contains("razer"));
    }

    #[test]
    fn finds_existing_dir_by_vendor_product_id() {
        let tmp = tempfile::tempdir().unwrap();
        let dev = device(0x5426, 0x103, "Razer Naga Trinity");

        let (first_handle, ..) = DeviceHandle::load_in(tmp.path(), &dev, test_macros()).unwrap();
        write_profile(&first_handle, "default", "# empty profile\n");

        let (second_handle, _, profiles) =
            DeviceHandle::load_in(tmp.path(), &dev, test_macros()).unwrap();

        assert_eq!(first_handle.dir(), second_handle.dir());
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "default");
    }

    #[test]
    fn find_if_device_name_changed() {
        let tmp = tempfile::tempdir().unwrap();

        let old_name = device(0x5426, 0x103, "Old device name");
        let (first_handle, ..) =
            DeviceHandle::load_in(tmp.path(), &old_name, test_macros()).unwrap();

        let new_name = device(0x5426, 0x103, "New device name");
        let (second_handle, ..) =
            DeviceHandle::load_in(tmp.path(), &new_name, test_macros()).unwrap();

        assert_eq!(first_handle.dir(), second_handle.dir());
    }

    #[test]
    fn find_if_dir_name_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let dev = device(0x5426, 0x103, "Razer Naga Trinity");
        let (handle, ..) = DeviceHandle::load_in(tmp.path(), &dev, test_macros()).unwrap();
        let renamed = tmp.path().join("renamed");

        std::fs::rename(handle.dir(), &renamed).unwrap();

        let (found_handle, ..) = DeviceHandle::load_in(tmp.path(), &dev, test_macros()).unwrap();

        assert_eq!(found_handle.dir(), renamed);
    }

    #[test]
    fn file_named_after_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let dev = device(0x5426, 0x103, "Razer Naga Trinity");
        let (handle, ..) = DeviceHandle::load_in(tmp.path(), &dev, test_macros()).unwrap();
        let path = write_profile(&handle, "gaming", "key = \"value\"\n");

        assert_eq!(path, handle.dir().join("gaming.toml"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "key = \"value\"\n");
    }

    #[test]
    fn remove_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dev = device(0x5426, 0x103, "Razer Naga Trinity");
        let (handle, ..) = DeviceHandle::load_in(tmp.path(), &dev, test_macros()).unwrap();

        write_profile(&handle, "temp", "");
        assert!(handle.dir().join("temp.toml").exists());

        handle.delete_profile("temp").unwrap();
        assert!(!handle.dir().join("temp.toml").exists());
    }
}
