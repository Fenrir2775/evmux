use crate::config::device_config::DeviceConfig;
use crate::config::device_handle::DeviceHandle;
use crate::config::profile;
use crate::config::profile::Profile;
use crate::device::input_device::InputDevice;
use crate::input::input_runtime::InputRuntime;
use crate::output::action::Actions;
use crate::session::session_command::SessionCommand;
use anyhow::Result;
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A session manages a single physical device.
pub(crate) struct DeviceSession {
    handle: DeviceHandle,
    config: DeviceConfig,
    profiles: HashMap<String, Arc<Profile>>,
    active_profile: Option<String>,
    runtime: Option<InputRuntime>,
}

impl DeviceSession {
    pub(crate) fn new(handle: DeviceHandle, config: DeviceConfig, profiles: Vec<Profile>) -> Self {
        let profiles = Self::build_profiles(profiles);
        let active_profile = Self::resolve_active(&config, None, &profiles);

        Self {
            handle,
            config,
            profiles,
            active_profile,
            runtime: None,
        }
    }

    pub(crate) fn is_running(&self) -> bool {
        self.runtime.is_some()
    }

    pub(crate) fn device(&self) -> &InputDevice {
        &self.config.device
    }

    pub(crate) fn config_dir(&self) -> &Path {
        self.handle.dir()
    }

    pub(crate) fn profiles(&self) -> Vec<String> {
        self.profiles.keys().cloned().collect()
    }

    pub(crate) fn active_profile(&self) -> Option<String> {
        self.active_profile.clone()
    }

    /// Send commands to the session.
    pub(crate) fn send_command(
        &mut self,
        cmd: SessionCommand,
        output_tx: Sender<Actions>,
    ) -> Result<()> {
        match cmd {
            SessionCommand::Start { profile } => self.start(output_tx, profile.as_deref()),
            SessionCommand::Stop => {
                self.stop();

                Ok(())
            }
            SessionCommand::SwitchProfile { name } => {
                self.switch_profile(&name)?;
                self.restart(output_tx, Some(&name))?;

                Ok(())
            }
            SessionCommand::RemoveProfile { name } => {
                self.remove_profile(&name)?;
                self.restart(output_tx, Some(&name))?;

                Ok(())
            }
            SessionCommand::Reload => {
                self.reload()?;
                self.restart(output_tx, self.active_profile.clone().as_deref())?;

                Ok(())
            }
        }
    }

    fn start(&mut self, output_tx: Sender<Actions>, profile: Option<&str>) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }

        let profile = match profile {
            Some(name) => self.find_profile(name)?,
            None => self.get_active_profile()?,
        };
        let name = profile.name.clone();

        self.runtime = Some(InputRuntime::start(
            &self.config.device,
            profile,
            output_tx,
        )?);
        self.active_profile = Some(name);

        Ok(())
    }

    fn stop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.stop()
        }
    }

    fn restart(&mut self, output_tx: Sender<Actions>, profile: Option<&str>) -> Result<()> {
        if self.is_running() {
            self.stop();
            self.start(output_tx, profile)?
        }

        Ok(())
    }

    /// Adds a new profile to the device, if it not exists.
    ///
    /// With `copy_from = Some(name)`, the profile is cloned under the new name.
    ///
    /// Returns the path to the profile file if successful.
    pub(crate) fn add_profile(&mut self, name: &str, copy_from: Option<&str>) -> Result<PathBuf> {
        if self.profiles.contains_key(name) {
            anyhow::bail!("Profile '{name}' already exists");
        }

        let content = match copy_from {
            None => profile::profile_template(name),
            Some(to_copy) => {
                let source_profile = self.find_profile(to_copy)?;
                let mut cloned = (*source_profile).clone();
                cloned.name = name.to_owned();
                toml::to_string_pretty(&cloned).map_err(|e| {
                    anyhow::anyhow!("Failed to serialize profile '{to_copy}': {e:#}")
                })?
            }
        };

        let path = self.handle.write_profile_content(name, &content)?;

        self.reload()?;

        Ok(path)
    }

    fn remove_profile(&mut self, name: &str) -> Result<()> {
        if !self.profiles.contains_key(name) {
            anyhow::bail!("Profile '{name}' doesn't exists");
        }

        self.handle.delete_profile(name)?;
        self.reload()
    }

    fn switch_profile(&mut self, name: &str) -> Result<()> {
        if !self.profiles.contains_key(name) {
            anyhow::bail!("Profile '{name}' doesn't exists");
        }

        self.active_profile = Some(name.to_owned());
        Ok(())
    }

    /// Reloads the device config from disk.
    ///
    /// Tries to keep the previously active profile active.
    fn reload(&mut self) -> Result<()> {
        let current = self.active_profile.clone();
        let (config, profiles) = self.handle.reload_device_config(&self.config.device)?;
        let profiles = Self::build_profiles(profiles);
        let active = Self::resolve_active(&config, current, &profiles);

        self.config = config;
        self.profiles = profiles;
        self.active_profile = active;

        Ok(())
    }

    fn find_profile(&self, name: &str) -> Result<Arc<Profile>> {
        self.profiles
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Profile '{name}' not found"))
    }

    fn get_active_profile(&self) -> Result<Arc<Profile>> {
        let name = self
            .active_profile
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No active profile"))?;

        self.find_profile(name)
    }

    /// Creates the `Hashmap<profile_name : profile>` from a profile list.
    fn build_profiles(profiles: Vec<Profile>) -> HashMap<String, Arc<Profile>> {
        profiles
            .into_iter()
            .map(|p| (p.name.clone(), Arc::new(p)))
            .collect()
    }

    /// Picks the active profile by order:
    /// 1. The profile that was already active, if it still exists.
    /// 2. The device's `default_profile`, if it still exists.
    /// 3. The first remaining profile.
    fn resolve_active(
        config: &DeviceConfig,
        current_profile: Option<String>,
        profiles: &HashMap<String, Arc<Profile>>,
    ) -> Option<String> {
        let is_valid = |name: &str| profiles.contains_key(name);

        current_profile
            .filter(|n| is_valid(n))
            .or_else(|| config.default_profile.clone().filter(|n| is_valid(n)))
            .or_else(|| profiles.keys().min().cloned())
    }

    pub(crate) fn matches(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        let dev = &self.config.device;

        dev.name().to_lowercase().contains(&q) || dev.physical_path().to_lowercase().contains(&q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::input_device::InputDevice;
    use tempfile::tempdir;

    fn test_session() -> DeviceSession {
        let tmp = tempdir().unwrap();
        let device = InputDevice::default();
        let (handle, config, profiles) = DeviceHandle::load_for_test(tmp.path(), &device).unwrap();

        DeviceSession::new(handle, config, profiles)
    }

    #[test]
    fn add_profile() {
        let mut session = test_session();

        assert!(session.profiles.is_empty());

        session.add_profile("foo", None).unwrap();
        session.add_profile("bar", None).unwrap();

        assert_eq!(session.profiles.len(), 2);
    }

    #[test]
    fn add_profile_duplicate_fails() {
        let mut session = test_session();

        session.add_profile("foo", None).unwrap();
        let result = session.add_profile("foo", None);

        assert!(result.is_err());
    }

    #[test]
    fn remove_profile() {
        let mut session = test_session();

        session.add_profile("foo", None).unwrap();
        session.add_profile("bar", None).unwrap();
        session.remove_profile("foo").unwrap();

        assert_eq!(session.profiles.len(), 1);
        assert!(!session.profiles.contains_key("foo"));
    }

    #[test]
    fn remove_non_existent_profile_fails() {
        let mut session = test_session();

        let err = session.remove_profile("foo");
        assert!(err.is_err());
    }

    #[test]
    fn switch_profile() {
        let mut session = test_session();

        session.add_profile("foo", None).unwrap();
        session.add_profile("bar", None).unwrap();

        session.switch_profile("bar").unwrap();
        assert_eq!(session.active_profile, Some("bar".to_string()));

        session.switch_profile("foo").unwrap();
        assert_eq!(session.active_profile, Some("foo".to_string()));
    }

    #[test]
    fn switch_to_non_existent_profile_fails() {
        let mut session = test_session();

        session.add_profile("foo", None).unwrap();

        let err = session.switch_profile("bar");
        assert!(err.is_err());
    }

    #[test]
    fn active_profile_follows_switch_and_remove() {
        let mut session = test_session();

        session.add_profile("foo", None).unwrap();
        session.add_profile("bar", None).unwrap();

        assert_eq!(session.active_profile, Some("foo".to_string()));

        session.switch_profile("bar").unwrap();
        assert_eq!(session.active_profile, Some("bar".to_string()));

        session.remove_profile("bar").unwrap();
        assert_eq!(session.active_profile, Some("foo".to_string()));
    }
}
