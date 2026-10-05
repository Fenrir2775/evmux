use crate::config;
use anyhow::{Context, Result};
use crossbeam_channel::Sender;
use inotify::{Event, EventMask, Inotify, WatchDescriptor, WatchMask};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::thread;

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub(crate) enum WatchEvent {
    /// Occurs if a `.toml` file change inside a device directory.
    DeviceConfigChanged(PathBuf),
    /// Occurs if a device directory (dis)appears under the root.
    DeviceDirChanged,
    /// Occurs if a macro file changes.
    MacroChanged,
}

enum ContentKind {
    Device,
    Macro,
}

struct ContentWatch {
    path: PathBuf,
    kind: ContentKind,
}

struct Watcher {
    inotify: Inotify,
    devices_root_wd: WatchDescriptor,
    content_watches: HashMap<WatchDescriptor, ContentWatch>,
}

impl Watcher {
    fn new() -> Result<Self> {
        let inotify = Inotify::init()?;
        // watch out for create, delete and move events in the /devices directory
        let devices_root_wd = inotify
            .watches()
            .add(
                config::devices_dir(),
                WatchMask::CREATE | WatchMask::DELETE | WatchMask::MOVE,
            )
            .context("Failed to add watch to devices root directory")?;

        let mut watcher = Self {
            inotify,
            devices_root_wd,
            content_watches: HashMap::new(),
        };

        watcher.watch_existing_device_dirs()?;
        watcher.add_content_watch(config::macros_dir(), ContentKind::Macro)?;
        Ok(watcher)
    }

    fn watch_existing_device_dirs(&mut self) -> Result<()> {
        for entry in std::fs::read_dir(config::devices_dir())? {
            let path = entry?.path();

            if path.is_dir() {
                self.add_content_watch(path, ContentKind::Device)?;
            }
        }

        Ok(())
    }

    fn add_content_watch(&mut self, path: PathBuf, kind: ContentKind) -> Result<()> {
        let wd = self
            .inotify
            .watches()
            .add(
                &path,
                WatchMask::CLOSE_WRITE | WatchMask::MOVED_TO | WatchMask::DELETE,
            )
            .with_context(|| format!("Failed to add inotify watch on: {path:?}"))?;

        self.content_watches.insert(wd, ContentWatch { path, kind });

        Ok(())
    }

    fn remove_content_watch(&mut self, path: &Path) {
        let Some(wd) = self
            .content_watches
            .iter()
            .find(|(_, cw)| cw.path == path)
            .map(|(wd, _)| wd.clone())
        else {
            return;
        };

        self.content_watches.remove(&wd);
        let _ = self.inotify.watches().remove(wd);
    }

    /// Return events if some are available.
    fn next_events(&mut self, buffer: &mut [u8]) -> Result<Vec<WatchEvent>> {
        let events = self.inotify.read_events_blocking(buffer)?;
        let mut out = vec![];

        for event in events {
            if event.wd == self.devices_root_wd {
                if let Some(ev) = self.handle_directory_event(&event) {
                    out.push(ev);
                }
            } else if let Some(ev) = self.handle_content_event(&event) {
                out.push(ev);
            }
        }

        Ok(out)
    }

    fn handle_directory_event(&mut self, event: &Event<&OsStr>) -> Option<WatchEvent> {
        if !event.mask.contains(EventMask::ISDIR) {
            return None;
        }

        let name = event.name?;
        let dir = config::devices_dir().join(name);

        if event
            .mask
            .intersects(EventMask::CREATE | EventMask::MOVED_TO)
        {
            if let Err(e) = self.add_content_watch(dir.clone(), ContentKind::Device) {
                eprintln!("Failed to watch new device dir '{}': {e:#}", dir.display());
            } else if event
                .mask
                .intersects(EventMask::DELETE | EventMask::MOVED_FROM)
            {
                self.remove_content_watch(&dir);
            }
        }

        Some(WatchEvent::DeviceDirChanged)
    }

    fn handle_content_event(&mut self, event: &Event<&OsStr>) -> Option<WatchEvent> {
        if event.mask.contains(EventMask::ISDIR) {
            return None;
        }

        let watch = self.content_watches.get(&event.wd)?;
        let file_name = event.name?.to_str()?;

        if !file_name.ends_with(".toml") {
            return None;
        }

        Some(match watch.kind {
            ContentKind::Device => WatchEvent::DeviceConfigChanged(watch.path.join(file_name)),
            ContentKind::Macro => WatchEvent::MacroChanged,
        })
    }
}

pub(crate) fn spawn_watcher(tx: Sender<WatchEvent>) -> Result<()> {
    thread::spawn(move || {
        if let Err(e) = run(tx) {
            eprintln!("Config watcher error: {e:#}");
        }
    });

    Ok(())
}

fn run(tx: Sender<WatchEvent>) -> Result<()> {
    let mut watcher = Watcher::new()?;
    let mut buffer = [0; 1024];

    loop {
        for event in watcher.next_events(&mut buffer)? {
            tx.send(event)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use crossbeam_channel::unbounded;
    use std::time::Duration;
    use serial_test::serial;
    use tempfile::TempDir;

    fn init_watcher(tx: Sender<WatchEvent>) -> Result<TempDir> {
        let tmp = tempfile::tempdir()?;

        unsafe { std::env::set_var(config::CONFIG_DIR, tmp.path()) };
        config::ensure_config_dir()?;
        spawn_watcher(tx)?;

        thread::sleep(Duration::from_millis(100));

        Ok(tmp)
    }

    #[test]
    #[serial]
    fn detect_new_device_dir() {
        let (tx, rx) = unbounded();
        let _tmp = init_watcher(tx).unwrap();

        std::fs::create_dir(config::devices_dir().join("test_device")).unwrap();
        let event = rx.recv_timeout(Duration::from_secs(1)).unwrap();

        assert!(matches!(event, WatchEvent::DeviceDirChanged));

        unsafe { std::env::remove_var(config::CONFIG_DIR) };
    }

    #[test]
    #[serial]
    fn detect_remove_device_dir() {
        let (tx, rx) = unbounded();
        let _tmp = init_watcher(tx).unwrap();

        let test_dir = config::devices_dir().join("test_device");

        std::fs::create_dir(&test_dir).unwrap();
        let event = rx.recv_timeout(Duration::from_secs(1)).unwrap();

        assert!(matches!(event, WatchEvent::DeviceDirChanged));

        std::fs::remove_dir(test_dir).unwrap();
        let event = rx.recv_timeout(Duration::from_secs(1)).unwrap();

        assert!(matches!(event, WatchEvent::DeviceDirChanged));

        unsafe { std::env::remove_var(config::CONFIG_DIR) };
    }

    #[test]
    #[serial]
    fn detect_move_device_dir() {
        let (tx, rx) = unbounded();
        let tmp = init_watcher(tx).unwrap();

        let test_dir = config::devices_dir().join("test_device");

        std::fs::create_dir(tmp.path().join("test_device")).unwrap();
        let event = rx.recv_timeout(Duration::from_secs(1));
        assert!(event.is_err());

        std::fs::rename(tmp.path().join("test_device"), test_dir).unwrap();
        let event = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(event, WatchEvent::DeviceDirChanged));

        unsafe { std::env::remove_var(config::CONFIG_DIR) };
    }

    #[test]
    #[serial]
    fn detect_config_file_changed() {
        let (tx, rx) = unbounded();
        let _tmp = init_watcher(tx).unwrap();

        let test_dir = config::devices_dir().join("test_device");
        std::fs::create_dir(&test_dir).unwrap();

        let dir_event = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(dir_event, WatchEvent::DeviceDirChanged));

        let test_toml = test_dir.join("test_config.toml");
        std::fs::write(test_toml, "test").unwrap();

        let config_event = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(&config_event, WatchEvent::DeviceConfigChanged(_)));

        unsafe { std::env::remove_var(config::CONFIG_DIR) };
    }

    #[test]
    #[serial]
    fn dont_trigger_non_config_toml() {
        let (tx, rx) = unbounded();
        let _tmp = init_watcher(tx).unwrap();

        let test_dir = config::devices_dir().join("test_device");
        std::fs::create_dir(&test_dir).unwrap();

        let dir_event = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(dir_event, WatchEvent::DeviceDirChanged));

        let test_toml = test_dir.join("test_config.txt");
        std::fs::write(test_toml, "test").unwrap();

        let config_event = rx.recv_timeout(Duration::from_secs(1));
        assert!(config_event.is_err());

        let another_test_dir = test_dir.join("another_test_dir");
        std::fs::create_dir(&another_test_dir).unwrap();

        let another_dir_event = rx.recv_timeout(Duration::from_secs(1));
        assert!(another_dir_event.is_err());

        unsafe { std::env::remove_var(config::CONFIG_DIR) };
    }

    #[test]
    #[serial]
    fn detect_macro_file_changed() {
        let (tx, rx) = unbounded();
        let _tmp = init_watcher(tx).unwrap();

        let test_toml = config::macros_dir().join("test_macro.toml");
        std::fs::write(test_toml, "test").unwrap();

        let macro_event = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(macro_event, WatchEvent::MacroChanged));

        unsafe { std::env::remove_var(config::CONFIG_DIR) };
    }

    #[test]
    #[serial]
    fn dont_trigger_non_macro_toml() {
        let (tx, rx) = unbounded();
        let _tmp = init_watcher(tx).unwrap();

        let test_toml = config::macros_dir().join("test_macro.txt");
        std::fs::write(test_toml, "test").unwrap();

        let macro_event = rx.recv_timeout(Duration::from_secs(1));
        assert!(macro_event.is_err());

        let test_dir = config::macros_dir().join("sub_macro");
        std::fs::create_dir(test_dir).unwrap();

        let dir_event = rx.recv_timeout(Duration::from_secs(1));
        assert!(dir_event.is_err());

        unsafe { std::env::remove_var(config::CONFIG_DIR) };
    }
}
