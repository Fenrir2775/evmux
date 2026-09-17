# `evmux`

## Introduction

`evmux` is a Linux input remapper for keyboards and other input devices.

It intercepts input events from physical devices and maps them to configurable actions
through a virtual output device.</br>
Every physical device gets its own configuration directory.</br>

It can also run as a daemon: either as a systemd system or user service,</br>
whichever you prefer.

***Currently supported actions:***
- Key to key
- Key to multiple
- Key to macro
- Suppressing keys
- Recording key input from a device, to find out which key you want to remap

## Installation

**`evmux` accesses Linux input devices directly. The user running `evmux` needs read access
to `/dev/input/event*` and write access to `/dev/uinput`.**

### Build yourself

***1. Build and install the binary:***

```bash
cargo build --release --locked
sudo install -Dm755 target/release/evmux /usr/local/bin/evmux
```

#### *2. Create an udev rule granting the `input` group access to `/dev/uinput`:*

`/etc/udev/rules.d/99-evmux-uinput.rules`

```text
KERNEL=="uinput", SUBSYSTEM=="misc", GROUP="input", MODE="0660"
```

***3. Reload udev rules:***

```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

***4. Add your user to the `input` group and re-login:***

`sudo usermod -aG input "$USER"`

#### *5. Create a systemd user service, `~/.config/systemd/user/evmux.service`:*

```ini
[Unit]
Description=evmux input remapping daemon

[Service]
ExecStart=/usr/local/bin/evmux
Restart=on-failure

[Install]
WantedBy=default.target
```

***6. Enable it***

`systemctl --user enable --now evmux.service`

## Usage

`evmux <COMMAND>`

Commands that operate on a device accept a case-insensitive substring of the device
name.</br>
For example:</br>
`evmux start razer`</br>
can select a device named `Razer Naga Pro`, the
complete device name does not have to be specified.

If no device matches the query, `evmux` returns an error.</br>
If multiple devices match, it returns an ambiguity error
containing the names and physical paths of all matching devices,</br>
no device is selected automatically.

### Device commands

| Command | Description |
|---|---|
| `evmux list` | List devices and profiles |
| `evmux start <DEVICE>` | Start remapping with the current profile |
| `evmux start <DEVICE> -p, --profile default` | Start remapping with the specified profile |
| `evmux stop <DEVICE>` | Stop remapping |
| `evmux record <DEVICE>` | Record the next key input from the specified device and print it |

### Profile commands

| Command | Description |
|---|---|
| `evmux profile add <DEVICE> <NAME>` | Add a profile |
| `evmux profile add razer gaming -c, --copy-from default` | An existing profile can be used as a template |
| `evmux profile remove <DEVICE> <NAME>` | Remove a profile |
| `evmux profile switch <DEVICE> <PROFILE>` | Switch profile |

## Configuration

Configuration files are stored in:

```
~/.config/evmux/
```

Device-specific configuration is stored below the `devices` directory:

```
~/.config/evmux/
└── devices/
    ├── <device>/
    │   ├── device.toml
    │   ├── <profile>.toml
    │   └── ...
    └── ...
```

## Planned:

- Axis remapping
- Support for additional input event types
- Macro scripting
- GUI
