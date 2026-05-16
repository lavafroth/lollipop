use evdev::{AbsoluteAxisCode, Device, EventStream, InputEvent, KeyEvent, LedCode, LedEvent};
use std::collections::BTreeMap;
use std::fmt::Display;
use std::fs::{File, OpenOptions};
use std::io::{self, Seek, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
mod config;
mod touchpad;
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, KeyCode};

use crate::config::key_code_to_modifier_name;
mod key_codes;

mod key_state;

pub struct InternalState {
    modifiers: BTreeMap<KeyCode, key_state::KeyState>,
    timeout: Duration,
    clear_all_with_escape: bool,
    touchpad: touchpad::Touchpad,
}

impl Display for InternalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (keycode, state) in self.modifiers.iter() {
            let Some(key_name) = key_code_to_modifier_name(*keycode) else {
                continue;
            };

            match state {
                key_state::KeyState::Latched(_system_time) => write!(f, "{key_name} ")?,
                key_state::KeyState::Locked => write!(f, "<b>{key_name}</b> ")?,
                _ => {}
            }
        }
        Ok(())
    }
}

impl InternalState {
    fn release_latched(&mut self) -> Vec<InputEvent> {
        let mut events = vec![];
        for (key, key_state) in self.modifiers.iter_mut() {
            if let key_state::KeyState::Latched(_) = key_state {
                *key_state = key_state::KeyState::None;
                events.push(*KeyEvent::new(*key, 0));
            }
        }
        self.touchpad.state = touchpad::TouchState::Idle;
        events
    }
    fn transition(&mut self, key: KeyCode, pressed: i32, timestamp: SystemTime) -> Vec<InputEvent> {
        let mut events = vec![];

        if self.clear_all_with_escape && key == KeyCode::KEY_ESC {
            for (key, key_state) in self.modifiers.iter_mut() {
                if !key_state::KeyState::None.eq(key_state) {
                    *key_state = key_state::KeyState::None;
                    events.push(*KeyEvent::new(*key, 0));
                }
            }
            return events;
        }

        if let Some(key_state) = self.modifiers.get_mut(&key) {
            if pressed == 1 {
                key_state.transition(timestamp, self.timeout);
            }
            return vec![*KeyEvent::new(key, key_state.pressed_state())];
        };

        events.push(*KeyEvent::new(key, pressed));
        events.extend_from_slice(&self.release_latched());
        events
    }

    fn led_state(&self) -> i32 {
        if self.modifiers.values().any(|v| v.pressed_state() > 0) {
            i32::MAX
        } else {
            0
        }
    }
}

fn pick_device() -> Result<Device, Error> {
    evdev::enumerate()
        .map(|(_, device)| device)
        .find(|d| d.name().is_some_and(|name| name.contains("keyboard")))
        .ok_or(Error::NoKeyboardDevice)
}

fn pick_touchpad() -> Result<Device, Error> {
    evdev::enumerate()
        .map(|(_, device)| device)
        .find(|d| {
            d.name()
                .is_some_and(|name| name.to_lowercase().contains("touchpad"))
        })
        .ok_or(Error::NoKeyboardDevice)
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to open a handle to keyboard device at path {path:?}: {io}")]
    OpenDeviceHandle { io: std::io::Error, path: String },

    #[error("no keyboard device available to augment input keypresses of")]
    NoKeyboardDevice,

    #[error(
        "invalid modifier {0:?} supplied in config, valid modifiers are: leftshift, rightshift, leftctrl, rightctrl, compose, leftmeta, fn, capslock, rightmeta"
    )]
    InvalidModifier(String),
    #[error(
        "invalid locking timeout {0:?} supplied, must be a positive integer for the number of milliseconds"
    )]
    InvalidTimeout(String),
    #[error(
        "invalid slop threshold {0:?} supplied, must be a positive integer for the small movements acceptable during a tap"
    )]
    InvalidSlop(String),

    #[error("invalid line in encoutered config: {0:?}")]
    InvalidConfig(String),

    #[error("failed to read config file {path:?}: {io}")]
    FailedReadingConfig { io: std::io::Error, path: String },
}

fn open_device(path: &str) -> Result<(Device, Device), Error> {
    Ok((
        Device::open(path).map_err(|io| Error::OpenDeviceHandle {
            io,
            path: path.to_string(),
        })?,
        Device::open(path).map_err(|io| Error::OpenDeviceHandle {
            io,
            path: path.to_string(),
        })?,
    ))
}

async fn handle_touchpad(
    touchpad_events: Option<&mut EventStream>,
) -> Option<io::Result<InputEvent>> {
    Some(touchpad_events?.next_event().await)
}

pub enum MaybeSharedMemory {
    Some(File),
    None,
}

impl MaybeSharedMemory {
    fn write_to_shm(&mut self, string: &str) -> io::Result<()> {
        match self {
            MaybeSharedMemory::Some(shared_memory) => {
                shared_memory.set_len(0)?;
                shared_memory.seek(io::SeekFrom::Start(0))?;
                shared_memory.write_all(string.as_bytes())?;
            }
            MaybeSharedMemory::None => {}
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let config = match std::env::args().nth(1) {
        Some(config_file) => config::Config::try_from_path(&config_file)?,
        None => config::Config::default(),
    };

    let (mut keyboard, mut led_sink) = if let Some(device_path) = config.keyboard_device {
        open_device(&device_path)?
    } else {
        (pick_device()?, pick_device()?)
    };

    let mut touchpad_events = if config.touchpad {
        Some(pick_touchpad()?.into_event_stream()?)
    } else {
        None
    };

    while keyboard.grab().is_err() {}

    println!("Taking over {}", keyboard.name().unwrap_or("keyboard"));
    let keys: AttributeSet<KeyCode> = key_codes::ALL.iter().collect();
    let mut lollipop_virtual_device = VirtualDevice::builder()?
        .name("lollipop")
        .with_keys(&keys)?
        .build()?;

    for path in lollipop_virtual_device.enumerate_dev_nodes_blocking()? {
        println!("Available as {}", path?.display());
    }

    let shared_memory_path = PathBuf::from("/dev/shm/lollipop.shm");
    if shared_memory_path.exists() {
        std::fs::remove_file(&shared_memory_path)?;
    }

    let mut shared_memory = if config.shm {
        MaybeSharedMemory::Some(
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o644)
                .open(shared_memory_path)?,
        )
    } else {
        MaybeSharedMemory::None
    };

    let mut state = InternalState {
        clear_all_with_escape: config.clear_all_with_escape,
        modifiers: BTreeMap::default(),
        timeout: Duration::from_millis(config.timeout),
        touchpad: touchpad::Touchpad {
            timeout: Duration::from_millis(config.touchpad_timeout),
            position: touchpad::POSITION_EMPTY,
            slop: config.touchpad_slop,
            state: touchpad::TouchState::Idle,
        },
    };

    for key in config.modifiers {
        state.modifiers.insert(key, key_state::KeyState::None);
    }

    let mut keyboard_events = keyboard.into_event_stream()?;

    loop {
        tokio::select! {
            _ = state.touchpad.timeout() => {
                lollipop_virtual_device.emit(&state.release_latched())?;
                led_sink.send_events(&[*LedEvent::new(LedCode::LED_CAPSL, state.led_state())])?;
                shared_memory.write_to_shm(&state.to_string())?;
            }

            Ok(event) = keyboard_events.next_event() => {
                if let evdev::EventSummary::Key(key_event, key_code, pressed) = event.destructure() {
                    let events = state.transition(key_code, pressed, key_event.timestamp());
                    // println!("{state:#?}");
                    lollipop_virtual_device.emit(&events)?;
                    led_sink.send_events(&[*LedEvent::new(LedCode::LED_CAPSL, state.led_state())])?;
                    shared_memory.write_to_shm(&state.to_string())?;
                }
            }

            Some(Ok(event)) = handle_touchpad(touchpad_events.as_mut()) => {

                if let evdev::EventSummary::Key(_key_event,
                    KeyCode::BTN_LEFT | KeyCode::BTN_RIGHT | KeyCode::BTN_TOUCH, pressed) = event.destructure() {
                    state.touchpad.respond_touch(pressed);
                    led_sink.send_events(&[*LedEvent::new(LedCode::LED_CAPSL, state.led_state())])?;
                    shared_memory.write_to_shm(&state.to_string())?;
                }
                if let evdev::EventSummary::AbsoluteAxis(_touchpad_event,
                    AbsoluteAxisCode::ABS_X | AbsoluteAxisCode::ABS_Y, xy) = event.destructure() {
                    state.touchpad.respond_motion(event.code() as usize, xy)
                }
            }
        }
    }
}
