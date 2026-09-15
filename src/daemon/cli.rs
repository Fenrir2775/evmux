use crate::daemon::ipc::Request;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "evmux",
    about = "Input remapper daemon and controller.",
    long_about = "Input remapper daemon and controller.\n\n\
        Device arguments support case-insensitive substring matching.\n\
        if multiple devices match, an error is returned listing the matching devices."
)]
struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List all discovered devices and profiles.
    List,
    /// Start remapping for a device.
    Start { device: String },
    /// Stop remapping for a device
    Stop { device: String },
    /// Manage profiles for a device.
    Profile {
        #[command(subcommand)]
        profile_command: ProfileCommand,
    },
    /// Record key input from a device
    Record { device: String },
}

#[derive(Subcommand)]
enum ProfileCommand {
    /// Create a new profile.
    Add {
        device: String,
        name: String,
        #[arg(long, value_name = "PROFILE")]
        copy_from: Option<String>,
    },
    /// Remove a profile.
    Remove { device: String, name: String },
    /// Switch to a named profile.
    Switch { device: String, profile: String },
}

pub(crate) fn parse_args() -> Option<Request> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::List) => Some(Request::ListDevices),
        Some(Command::Start { device }) => Some(Request::Start { device }),
        Some(Command::Stop { device }) => Some(Request::Stop { device }),
        Some(Command::Profile { profile_command }) => match profile_command {
            ProfileCommand::Add {
                device,
                name,
                copy_from,
            } => Some(Request::AddProfile {
                device,
                name,
                copy_from,
            }),
            ProfileCommand::Remove { device, name } => {
                Some(Request::RemoveProfile { device, name })
            }
            ProfileCommand::Switch { device, profile } => {
                Some(Request::SwitchProfile { device, profile })
            }
        },
        Some(Command::Record { device }) => Some(Request::Record { device }),
        None => None,
    }
}
