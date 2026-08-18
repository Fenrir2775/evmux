use crate::device::input_device::InputDevice;
use crate::input::reader::grabbed_device::GrabbedDevice;
use crate::input::reader::reader_thread::ReaderThread;
use anyhow::{Context, Result, anyhow};
use crossbeam_channel::Sender;
use evdev::{Device, InputEvent};
use nix::sys::epoll;
use nix::sys::eventfd::{EfdFlags, EventFd};
use std::sync::Arc;
use std::{io, thread};

/// Spawns one reader thread per evdev node belonging to [`InputDevice`].
pub(in crate::input) fn spawn_readers(
    device: &InputDevice,
    raw_tx: Sender<InputEvent>,
) -> Result<Vec<ReaderThread>> {
    let mut readers = vec![];
    for d in device.matching_devices() {
        let reader = spawn_reader_thread(d, raw_tx.clone())?;
        readers.push(reader);
    }

    Ok(readers)
}

fn spawn_reader_thread(device: Device, raw_tx: Sender<InputEvent>) -> Result<ReaderThread> {
    let epoll_fd = epoll::Epoll::new(epoll::EpollCreateFlags::EPOLL_CLOEXEC)
        .context("failed to create epoll instance")?;
    let stop_event =
        Arc::new(EventFd::from_flags(EfdFlags::EFD_NONBLOCK).context("failed to create eventfd")?);

    // for input events
    epoll_fd.add(
        &device,
        epoll::EpollEvent::new(epoll::EpollFlags::EPOLLIN, 0),
    )?;
    // for stop event
    epoll_fd.add(
        &stop_event,
        epoll::EpollEvent::new(epoll::EpollFlags::EPOLLIN, 1),
    )?;

    let join_handle = thread::spawn(move || {
        if let Err(e) = run_reader_loop(device, raw_tx, epoll_fd) {
            eprintln!("Reader thread stopped: {e:#}");
        }
    });

    Ok(ReaderThread::new(join_handle, stop_event))
}

fn run_reader_loop(
    device: Device,
    raw_tx: Sender<InputEvent>,
    epoll_fd: epoll::Epoll,
) -> Result<()> {
    let mut device = GrabbedDevice::new(device)?;
    let mut events = [epoll::EpollEvent::empty(); 2];

    loop {
        match device.fetch_events() {
            Ok(iterator) => {
                for event in iterator {
                    if raw_tx.send(event).is_err() {
                        return Ok(());
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                match epoll_fd.wait(&mut events, epoll::EpollTimeout::NONE) {
                    Ok(n) => {
                        if events[..n].iter().any(|e| e.data() == 1) {
                            return Ok(());
                        }
                    }
                    Err(e) => return Err(anyhow!(e)),
                }
            }
            Err(e) => anyhow::bail!("failed to fetch input events: {e}"),
        }
    }
}
