# Changelog

## Unreleased

## [v0.2.0] - 2026-10-06

### Added

- Rules for relative axes: `Invert/Swap/Scale`
- Added a global `macros` directory
- Inotify-watcher listen to the `macros` directory
- Profile and Rule now have a related parse-type
- Added environment-variable `EVMUX_CONFIG_DIR` for a custom configuration directoy

### Changed

- Command results print now the real device name, not the query
- Macros are compiled while parsing, not while matching anymore
- The command `evmux profile add <device> <profile> --copy-from` now just copies the profile
- changed method `relative_move` to a more general method `relative_axis`

### Fixed

- Watcher now returns the full path to the file

## [v0.1.1] - 2026-09-19

### Fixed

- match for nix::Errno::EINTR, reader thread shouldn't stop anymore after resume from standby

## [v0.1.0] - 2026-09-16

### Added

- Linux input device remapping via `evdev/uinput`
- Per-device configuration, stored under `~/.config/evmux/devices/<device>/`
- Profiles per device, switchable at runtime
- Rule types: key-to-key, key-to-multiple, key-blocking, macros
- CLI (`evmux list/start/stop/record/profile ...`) communicates with a daemon over a Unix socket
- Optional systemd user service
- Config hot-reload via inotify

[v0.2.0]: https://github.com/Fenrir2775/evmux/releases/tag/v0.2.0
[v0.1.1]: https://github.com/Fenrir2775/evmux/releases/tag/v0.1.1
[v0.1.0]: https://github.com/Fenrir2775/evmux/releases/tag/v0.1.0