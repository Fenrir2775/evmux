use std::io::{BufReader};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread::JoinHandle;
use std::time::Duration;
use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};
use anyhow::{anyhow, Context, Result};

/// Infos about a single device's state.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DeviceInfo {
    pub name: String,
    pub running: bool,
    pub profiles: Vec<String>,
    pub active_profile: Option<String>,
}

/// Requests sent to the daemon.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
pub(crate) enum Request {
    /// Starts remapping for the given device.
    Start { device: String },
    /// Stops remapping for the given device.
    Stop { device: String },
    /// Adds a new profile to a device,
    /// optionally copy from an already existing device.
    AddProfile {
        device: String,
        name: String,
        copy_from: Option<String>,
    },
    /// Removes a profile from a device.
    RemoveProfile {
        device: String,
        name: String,
    },
    /// Switch the active profile of the given device.
    SwitchProfile {
        device: String,
        profile: String,
    },
    /// List all found devices.
    ListDevices,
    /// Reload all configs from disk.
    Reload,
    Record { device: String },
}

/// Responses sent from the daemon.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub(crate) enum Response {
    Success { message: String },
    Error { message: String },
    /// Response to [`Request::ListDevices`]
    Devices { devices: Vec<DeviceInfo> },
}

/// Path of the Unix socket.
pub(crate) fn socket_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(fallback_dir)
        .join("evmux.sock")
}

fn fallback_dir() -> PathBuf {
    let dir = dirs::config_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("evmux");

    let _ = std::fs::create_dir_all(&dir);

    dir
}

/// Reads one request from stream and send it to the Sender channel.
fn receive_request(mut stream: UnixStream, tx: &Sender<(Request, UnixStream)>) -> Result<()> {
    let request = match serde_json::from_reader(BufReader::new(&stream)) {
        Ok(request) => request,
        Err(err) => {
            let response = Response::Error {
                message: format!("Invalid request: {err:#}"),
            };

            let _ = serde_json::to_writer(&mut stream, &response);
            return Err(anyhow!("Failed to deserialize request"));
        }
    };

    tx.send((request, stream))?;

    Ok(())
}

/// Connects to the running daemon, send one request and returns the response.
pub(crate) fn send_request(request: &Request) -> Result<Response> {
    let mut stream = UnixStream::connect(socket_path())
        .context("Could not connect to daemon. Is evmux running?")?;

    serde_json::to_writer(&mut stream, &request).context("Failed to send request")?;

    stream.shutdown(std::net::Shutdown::Write)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;

    serde_json::from_reader(BufReader::new(&stream)).context("Failed to read response")
}

/// Binds the unix socket and spawns the listener.
pub(crate) fn spawn_ipc_listener(tx: Sender<(Request, UnixStream)>) -> Result<JoinHandle<()>> {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)
        .with_context(|| format!("Could not bind to {}", path.to_string_lossy()))?;

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;

    Ok(std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
                        if let Err(e) = receive_request(stream, &tx) {
                            eprintln!("IPC connection error: {e:#}");
                        }
                    });
                }
                Err(e) => eprintln!("IPC accept error: {e}"),
            }
        }
    }))
}