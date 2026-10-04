mod config;
mod daemon;
mod device;
mod input;
mod output;
mod session;

use crate::config::watcher;
use crate::config::watcher::WatchEvent;
use crate::daemon::daemon::Daemon;
use crate::daemon::ipc::{Request, Response};
use crate::daemon::{cli, ipc};
use anyhow::{Result, bail, ensure};
use crossbeam_channel::select;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::os::unix::net::UnixStream;
use std::path::Path;

fn main() -> Result<()> {
    check_permissions()?;

    match cli::parse_args() {
        Some(request) => run_client(request),
        None => run_daemon(),
    }
}

fn run_client(request: Request) -> Result<()> {
    match ipc::send_request(&request)? {
        Response::Success { message } => println!("{message}"),
        Response::Error { message } => {
            eprintln!("Error: {message}");
            std::process::exit(1);
        }
        Response::Devices { devices } => {
            for device in devices {
                let running = if device.running { "running" } else { "stopped" };
                let active = device.active_profile.as_deref().unwrap_or("none");

                println!("{} [{running}] - active: {active}", device.name);

                for profile in &device.profiles {
                    println!("  - {profile}")
                }
            }
        }
    }

    Ok(())
}

fn run_daemon() -> Result<()> {
    if UnixStream::connect(ipc::socket_path()).is_ok() {
        bail!("daemon already running")
    }

    config::ensure_config_dir()?;

    let mut daemon = Daemon::new()?;
    let (watch_tx, watch_rx) = crossbeam_channel::unbounded();
    let (ipc_tx, ipc_rx) = crossbeam_channel::unbounded();
    
    watcher::spawn_watcher(watch_tx)?;    
    ipc::spawn_ipc_listener(ipc_tx)?;

    eprintln!("evmux daemon running.");

    loop {
        select! {
            recv(watch_rx) -> event => match event? {
                WatchEvent::DeviceConfigChanged(dir) => {
                    if let Err(e) = daemon.reload_by_path(&dir) {
                        eprintln!("Reload failed for {dir:?}: {e:#}");
                    }
                }
                WatchEvent::DeviceDirChanged | WatchEvent::MacroChanged => {
                    if let Err(e) = daemon.reload() {
                        eprintln!("Full reload failed: {e:#}");
                    }
                }
            },
            recv(ipc_rx) -> msg => {
                let (request, mut stream) = msg?;
                let response = daemon.handle_request(request);
                if let Err(e) = serde_json::to_writer(&mut stream, &response) {
                    eprintln!("Failed to send IPC response: {e:#}");
                }
            }
        }
    }
}

fn check_permissions() -> Result<()> {
    check_uinput()?;
    check_input()?;

    Ok(())
}

fn check_uinput() -> Result<()> {
    let path = Path::new("/dev/uinput");

    ensure!(
        path.exists(),
        "{path:?} does not exist. Please run 'sudo modprobe uinput'"
    );

    if let Err(e) = OpenOptions::new().write(true).open(path) {
        match e.kind() {
            ErrorKind::NotFound => {
                bail!("uinput kernel module not loaded. Please run 'sudo modprobe uinput'")
            }
            ErrorKind::PermissionDenied => bail!(
                "evmux cannot write to {path:?}.\n\
            Ensure your udev rules are applied and your user belongs to the `input` group."
            ),
            _ => bail!("Cannot access {path:?}: {e:#}"),
        }
    };

    Ok(())
}

fn check_input() -> Result<()> {
    let path = Path::new("/dev/input");
    let mut readable = false;

    for entry in std::fs::read_dir(path)? {
        let path = entry?.path();

        if path
            .file_name()
            .and_then(|f| f.to_str())
            .is_some_and(|f| f.starts_with("event"))
            && OpenOptions::new().read(true).open(path).is_ok()
        {
            readable = true;
            break;
        }
    }

    if !readable {
        bail!(
            "Cannot read input devices in {path:?}. Ensure your user belongs to the `input` group."
        );
    }

    Ok(())
}
