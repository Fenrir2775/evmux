use anyhow::{Context, Result};
use crossbeam_channel::Sender;
use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;

pub(crate) enum WatchEvent {
    /// Occurs if a `.toml` file change inside a device directory.
    DeviceConfigChanged(PathBuf),
    /// Occurs if a device directory (dis)appears under the root.
    DeviceDirChanged,
}

pub(crate) fn spawn_watcher(root: PathBuf, tx: Sender<WatchEvent>) -> Result<()> {
    thread::spawn(move || {
        if let Err(e) = run(root, tx) {
            eprintln!("Config watcher error: {e:#}");
        }
    });

    Ok(())
}

fn run(root: PathBuf, tx: Sender<WatchEvent>) -> Result<()> {
    std::fs::create_dir_all(&root)?;

    let mut inotify = Inotify::init()?;
    let mut watches: HashMap<WatchDescriptor, PathBuf> = HashMap::new();

    // watch for the root directory
    let root_wd = inotify
        .watches()
        .add(
            &root,
            WatchMask::CREATE | WatchMask::DELETE | WatchMask::MOVE,
        )
        .context("Failed to add watch to config root directory")?;

    // add watches for all device directories
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?.path();
        if entry.is_dir() {
            watch_device_dir(&mut inotify, &mut watches, entry)?;
        }
    }

    let mut buffer = [0; 1024];

    loop {
        let events = inotify.read_events_blocking(&mut buffer)?;

        for event in events {
            let name = event.name.map(|n| n.to_string_lossy().into_owned());

            if event.wd == root_wd {
                if !event.mask.contains(EventMask::ISDIR) {
                    continue;
                }

                // add to watcher if a new directory appears
                if event
                    .mask
                    .intersects(EventMask::CREATE | EventMask::MOVED_TO)
                {
                    if let Some(name) = name {
                        let dir = root.join(&name);
                        if let Err(e) = watch_device_dir(&mut inotify, &mut watches, dir) {
                            eprintln!("Failed to watch new device dir '{name}': {e:#}");
                        }
                    }
                    // or remove if one disappears
                } else if event
                    .mask
                    .intersects(EventMask::DELETE | EventMask::MOVED_FROM)
                    && let Some(name) = name
                {
                    watches.retain(|_, p| p != &root.join(&name));
                }

                tx.send(WatchEvent::DeviceDirChanged)?;
            } else if let Some(device_dir) = watches.get(&event.wd).cloned() {
                if event.mask.contains(EventMask::ISDIR) {
                    continue;
                }

                let Some(filename) = name else { continue };

                if !filename.ends_with(".toml") {
                    continue;
                }

                tx.send(WatchEvent::DeviceConfigChanged(device_dir))?
            }
        }
    }

    /// Add a device directory to the watcher
    fn watch_device_dir(
        inotify: &mut Inotify,
        watches: &mut HashMap<WatchDescriptor, PathBuf>,
        dir: PathBuf,
    ) -> Result<()> {
        let wd = inotify
            .watches()
            .add(
                &dir,
                WatchMask::CLOSE_WRITE | WatchMask::MOVED_TO | WatchMask::DELETE,
            )
            .with_context(|| format!("Failed to add inotify watch on {dir:?}"))?;

        watches.insert(wd, dir);

        Ok(())
    }
}
