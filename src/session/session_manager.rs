use crate::config::device_handle::DeviceHandle;
use crate::config::macros::Macros;
use crate::device::input_device;
use crate::device::input_device::InputDevice;
use crate::input::record;
use crate::output::output_runtime::OutputRuntime;
use crate::session::device_session::DeviceSession;
use crate::session::session_command::SessionCommand;
use anyhow::{Result, anyhow};
use evdev::KeyCode;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Controls all device sessions.
pub(crate) struct SessionManager {
    sessions: HashMap<InputDevice, DeviceSession>,
    output_runtime: OutputRuntime,
    macros: Arc<RwLock<Macros>>,
}

impl SessionManager {
    /// Creates a new session for every device, the `OutputRuntime` and load all macros from [`macros_dir`](crate::config::macros_dir).
    ///
    /// Bad configs are reported and skipped, because a broken config shouldn't prevent any other from working.
    pub(crate) fn new() -> Result<Self> {
        let macros = Arc::new(RwLock::new(Macros::load()?));
        let mut sessions = HashMap::new();
        let output_runtime = OutputRuntime::new()?;

        for device in input_device::enumerate_devices() {
            match DeviceHandle::load(&device, macros.clone()) {
                Ok((handle, config, profiles)) => {
                    sessions.insert(device, DeviceSession::new(handle, config, profiles));
                }
                Err(err) => eprintln!("Failed to load config for '{}': {err:#}", device.name()),
            }
        }

        Ok(Self {
            sessions,
            output_runtime,
            macros,
        })
    }

    /// Returns an iterator over all sessions.
    pub(crate) fn sessions(&self) -> impl Iterator<Item = &DeviceSession> {
        self.sessions.values()
    }

    fn dispatch(&mut self, device: &str, cmd: SessionCommand) -> Result<&DeviceSession> {
        let tx = self.output_runtime.sender();
        let session = self.session_mut(device)?;
        session.send_command(cmd, tx)?;

        Ok(session)
    }

    /// Starts the device using its currently active profile.
    pub(crate) fn start(&mut self, device: &str, profile: Option<String>) -> Result<&InputDevice> {
        let session = self.dispatch(device, SessionCommand::Start { profile })?;

        Ok(session.device())
    }

    /// Stops the device runtime.
    pub(crate) fn stop(&mut self, device: &str) -> Result<&InputDevice> {
        let session = self.dispatch(device, SessionCommand::Stop)?;

        Ok(session.device())
    }

    /// Reload configs for all devices and restart them if they were running.
    pub(crate) fn reload(&mut self) -> Result<()> {
        let macros = Macros::load()?;

        match self.macros.write() {
            Ok(mut lock) => *lock = macros,
            Err(e) => anyhow::bail!("macros lock poisoned: {e:#}"),
        }

        let devices = input_device::enumerate_devices();

        for device in self.sessions.keys().cloned().collect::<Vec<_>>() {
            if !devices.contains(&device)
                && let Some(mut session) = self.sessions.remove(&device)
            {
                let tx = self.output_runtime.sender();
                let _ = session.send_command(SessionCommand::Stop, tx);
            }
        }

        for device in devices {
            if let Some(session) = self.sessions.get_mut(&device) {
                let tx = self.output_runtime.sender();
                if let Err(e) = session.send_command(SessionCommand::Reload, tx) {
                    eprintln!("Failed to reload '{}': {e:#}", device.name())
                }
            } else {
                match DeviceHandle::load(&device, self.macros.clone()) {
                    Ok((handle, config, profiles)) => {
                        self.sessions
                            .insert(device, DeviceSession::new(handle, config, profiles));
                    }
                    Err(e) => eprintln!("Failed to load config for '{}': {e:#}", device.name()),
                }
            }
        }

        Ok(())
    }

    /// Reload the config for a given device.
    pub(crate) fn reload_by_path(&mut self, path: &Path) -> Result<()> {
        let device = self
            .sessions
            .iter()
            .find(|(_, s)| s.device_dir() == path)
            .map(|(d, _)| d.clone());

        if let Some(device) = device {
            self.dispatch(device.name(), SessionCommand::Reload)?;
        }

        Ok(())
    }

    /// Creates a new profile for the device and returns the path of the created file.
    ///
    /// This bypasses `SessionCommand` because it returns a PathBuf to the created file.
    pub(crate) fn add_profile(
        &mut self,
        device: &str,
        name: &str,
        copy_from: Option<&str>,
    ) -> Result<PathBuf> {
        self.session_mut(device)?.add_profile(name, copy_from)
    }

    /// Removes a profile from the device.
    pub(crate) fn remove_profile(&mut self, device: &str, profile: &str) -> Result<&InputDevice> {
        let session = self.dispatch(
            device,
            SessionCommand::RemoveProfile {
                profile: profile.to_string(),
            },
        )?;

        Ok(session.device())
    }

    /// Makes the given profile active.
    pub(crate) fn switch_profile(&mut self, device: &str, profile: &str) -> Result<&InputDevice> {
        let session = self.dispatch(
            device,
            SessionCommand::SwitchProfile {
                profile: profile.to_string(),
            },
        )?;

        Ok(session.device())
    }

    /// Records the next keypress of the given device.
    pub(crate) fn record(&mut self, query: &str) -> Result<KeyCode> {
        let session = self.session_mut(query)?;

        if session.is_running() {
            anyhow::bail!(
                "Cannot record while '{}' is active. Stop it first with: evmux stop \"{}\"",
                session.device().name(),
                session.device().name()
            );
        }

        let device = session.device();
        record::record_keypress(device)
    }

    /// Returns a mutable session for the given query (partial, case-insensitive).
    ///
    /// Matches against device name & physical path prefix.
    ///
    /// Errors if nothing matches or if more than one device matches.
    fn session_mut(&mut self, query: &str) -> Result<&mut DeviceSession> {
        let mut matches: Vec<_> = self
            .sessions
            .values_mut()
            .filter(|s| s.matches(query))
            .collect();

        match matches.len() {
            0 => Err(anyhow!("Device '{query}' not found")),
            1 => Ok(matches.remove(0)),
            _ => {
                let mut devices: Vec<_> = matches
                    .iter()
                    .map(|s| format!("{} ({})", s.device().name(), s.device().physical_path()))
                    .collect();
                devices.sort();

                Err(anyhow!(
                    "Device '{query}' is ambiguous, matches:\n {}",
                    devices.join("\n  ")
                ))
            }
        }
    }
}
