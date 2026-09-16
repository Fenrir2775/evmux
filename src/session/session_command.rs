/// Commands to control the [`DeviceSession`].
pub(crate) enum SessionCommand {
    /// Starts remapping, optionally with a profile.
    Start { profile: Option<String> },
    /// Stops remapping.
    Stop,
    /// Switch to the given profile.
    SwitchProfile { profile: String },
    /// Removes a profile file.
    RemoveProfile { profile: String },
    /// Reload the device Config from disk.
    Reload,
}
