use std::path::Path;
use crate::session::session_manager::SessionManager;
use anyhow::Result;
use crate::daemon::ipc::{DeviceInfo, Request, Response};

pub(crate) struct Daemon {
    manager: SessionManager,
}

impl Daemon {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            manager: SessionManager::new()?,
        })
    }

    /// Process incoming [`Request`]s and returns a [`Response`].
    pub(crate) fn handle_request(&mut self, request: Request) -> Response {
        match request {
            Request::ListDevices => Response::Devices {
                devices: self.device_infos(),
            },
            Request::Start { device } => Self::respond(self.manager.start(&device), |_| {
               format!("Started '{device}'")
            }),
            Request::Stop { device } => Self::respond(self.manager.stop(&device), |_| {
               format!("Stopped '{device}'")
            }),
            Request::SwitchProfile { device, profile } => {
                Self::respond(self.manager.switch_profile(&device, &profile), |_| {
                    format!("Switched '{device}' to profile '{profile}'")
                })
            },
            Request::AddProfile { device, name, copy_from } => {
                Self::respond(self.manager.add_profile(&device, &name, copy_from.as_deref()),
                |path| format!("Created profile '{name}' at {}", path.display()))
            },
            Request::RemoveProfile { device, name } => {
                Self::respond(self.manager.remove_profile(&device, &name), |_| {
                    format!("Removed profile '{name}', from {device}")
                })
            },
            Request::Reload => Self::respond(self.reload(), |_| {
                "Configuration reloaded".into()
            }),
            Request::Record { device } => Self::respond(self.manager.record(&device), |key| {
                format!("Detected: {key:?} (code: {})", key.0)
            })
        }
    }

    fn respond<T>(result: Result<T>, message: impl FnOnce(T) -> String) -> Response {
        match result {
            Ok(val) => Response::Success {
                message: message(val),
            },
            Err(e) => Response::Error {
                message: format!("{e:#}"),
            },
        }
    }

    /// Reloads all device configs from disk.
    pub(crate) fn reload(&mut self) -> Result<()> {
        self.manager.reload()
    }

    /// Reloads the config for a specific device.
    pub(crate) fn reload_by_path(&mut self, path: &Path) -> Result<()> {
        self.manager.reload_by_path(path)
    }

    /// Collect the current state of all sessions.
    fn device_infos(&self) -> Vec<DeviceInfo> {
        self.manager
            .sessions()
            .map(|session| DeviceInfo {
                name: session.device().name().to_owned(),
                running: session.is_running(),
                profiles: session.profiles(),
                active_profile: session.active_profile()
            })
            .collect()
    }
}