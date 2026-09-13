mod config;
mod daemon;
mod device;
mod input;
mod output;
mod session;

use crate::config::config_store::ConfigStore;
use crate::config::watcher;
use crate::config::watcher::WatchEvent;
use crate::daemon::daemon::Daemon;
use crate::daemon::ipc::{Request, Response};
use crate::daemon::{cli, ipc};
use anyhow::Result;
use crossbeam_channel::select;
use std::os::unix::net::UnixStream;

fn main() -> Result<()> {
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
        anyhow::bail!("daemon already running")
    }

    let mut daemon = Daemon::new()?;
    let (watch_tx, watch_rx) = crossbeam_channel::unbounded();
    watcher::spawn_watcher(ConfigStore::root_dir(), watch_tx)?;

    let (ipc_tx, ipc_rx) = crossbeam_channel::unbounded();
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
                WatchEvent::DeviceDirChanged => {
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
