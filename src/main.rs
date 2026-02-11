use evdev::{AbsoluteAxisCode, Device, EventStream, InputEvent, KeyEvent, LedCode, LedEvent};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::time::{Duration, SystemTime};
use std::{fs, io};

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, KeyCode};
mod key_codes;

#[derive(Clone, Copy, PartialEq)]
pub enum KeyState {
    Latched(SystemTime),
    Locked,
    None,
}

impl Debug for KeyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Latched(time) => write!(
                f,
                "Latched {}s",
                time.elapsed().unwrap_or_default().as_secs()
            ),
            Self::Locked => write!(f, "Locked"),
            Self::None => write!(f, "None"),
        }
    }
}

impl KeyState {
    fn transition(&mut self, time: SystemTime, timeout: Duration) {
        *self = match self {
            KeyState::Latched(last_press) => {
                if let Ok(elapsed) = time.duration_since(*last_press)
                    && elapsed < timeout
                {
                    KeyState::Locked
                } else {
                    KeyState::None
                }
            }
            KeyState::Locked => KeyState::None,
            KeyState::None => KeyState::Latched(time),
        }
    }

    fn pressed_state(&self) -> i32 {
        match self {
            KeyState::Locked | KeyState::Latched(_) => 1,
            KeyState::None => 0,
        }
    }
}

pub struct Touchpad {
    dragging: bool,
    position: [i32; 2],
    buffer: Vec<InputEvent>,
    last_release: Option<SystemTime>,
    fuzz: u64,
}

pub struct InternalState {
    modifiers: BTreeMap<KeyCode, KeyState>,
    timeout: Duration,
    clear_all_with_escape: bool,
    touchpad: Touchpad,
}

const TOUCH_RELEASED: i32 = 0;
const TOUCH_HELD: i32 = 1;
const COORDINATE_EMPTY: i32 = -1;
const POSITION_EMPTY: [i32; 2] = [-1, -1];

impl InternalState {
    fn release_latched(&mut self) -> Vec<InputEvent> {
        let mut events = vec![];
        for (key, key_state) in self.modifiers.iter_mut() {
            if let KeyState::Latched(_) = key_state {
                *key_state = KeyState::None;
                events.push(*KeyEvent::new(*key, 0));
            }
        }
        events
    }
    fn respond_touch(&mut self, touch: i32) {
        if touch == TOUCH_HELD {
            self.touchpad.dragging = false;
            self.touchpad.last_release = None;
        }

        if !self.touchpad.dragging && touch == TOUCH_RELEASED {
            self.touchpad.last_release = Some(SystemTime::now());
            self.touchpad.buffer = self.release_latched();
        }
    }
    fn respond_motion(&mut self, axis: usize, coordinate: i32) {
        if self.touchpad.dragging {
            return;
        }

        if self.touchpad.position[axis] == COORDINATE_EMPTY {
            self.touchpad.position[axis] = coordinate;
            return;
        }

        // if the cursor is pushed beyond a `fuzz` sided square
        // in the touchpad, it is getting dragged
        if (self.touchpad.position[axis] - coordinate).abs() as u64 > self.touchpad.fuzz {
            self.touchpad.dragging = true;
            self.touchpad.position = POSITION_EMPTY;
        }
    }
    fn transition(&mut self, key: KeyCode, pressed: i32, timestamp: SystemTime) -> Vec<InputEvent> {
        let mut events = vec![];

        if self.clear_all_with_escape && key == KeyCode::KEY_ESC {
            for (key, key_state) in self.modifiers.iter_mut() {
                if !KeyState::None.eq(key_state) {
                    *key_state = KeyState::None;
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

pub struct Config {
    modifiers: Vec<KeyCode>,
    timeout: u64,
    keyboard_device: Option<String>,
    clear_all_with_escape: bool,
    touchpad: bool,
    touchpad_timeout: u64,
    touchpad_fuzz: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            touchpad_fuzz: 300,
            clear_all_with_escape: true,
            modifiers: vec![
                KeyCode::KEY_LEFTSHIFT,
                KeyCode::KEY_LEFTMETA,
                KeyCode::KEY_LEFTCTRL,
                KeyCode::KEY_LEFTALT,
            ],
            timeout: 500,
            keyboard_device: None,
            touchpad: false,
            touchpad_timeout: 200,
        }
    }
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
        "invalid fuzz threshold {0:?} supplied, must be a positive integer for the small movements acceptable during a tap"
    )]
    InvalidFuzz(String),

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

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let config = match std::env::args().nth(1) {
        Some(config_file) => parse_config(&config_file)?,
        None => Config::default(),
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
        let path = path?;
        println!("Available as {}", path.display());
    }

    let mut state = InternalState {
        clear_all_with_escape: config.clear_all_with_escape,
        modifiers: BTreeMap::default(),
        timeout: Duration::from_millis(config.timeout),
        touchpad: Touchpad {
            dragging: false,
            position: [-1, -1],
            buffer: vec![],
            last_release: None,
            fuzz: config.touchpad_fuzz,
        },
    };

    for key in config.modifiers {
        state.modifiers.insert(key, KeyState::None);
    }

    let touchpad_timeout = Duration::from_millis(config.touchpad_timeout);

    let mut keyboard_events = keyboard.into_event_stream()?;

    loop {
        if state
            .touchpad
            .last_release
            .and_then(|v| v.elapsed().ok())
            .is_some_and(|v| v > touchpad_timeout)
        {
            lollipop_virtual_device.emit(&state.touchpad.buffer)?;
            state.touchpad.buffer.clear();
        }
        tokio::select! {
            Ok(event) = keyboard_events.next_event() => {
                if let evdev::EventSummary::Key(key_event, key_code, pressed) = event.destructure() {
                    let events = state.transition(key_code, pressed, key_event.timestamp());
                    // println!("{state:#?}");
                    lollipop_virtual_device.emit(&events)?;
                    led_sink.send_events(&[*LedEvent::new(LedCode::LED_CAPSL, state.led_state())])?;
                }
            }

            Some(Ok(event)) = handle_touchpad(touchpad_events.as_mut()) => {

                if let evdev::EventSummary::Key(_key_event, KeyCode::BTN_LEFT | KeyCode::BTN_RIGHT | KeyCode::BTN_TOUCH, pressed) = event.destructure() {
                    state.respond_touch(pressed);
                    led_sink.send_events(&[*LedEvent::new(LedCode::LED_CAPSL, state.led_state())])?;
                }
                if let evdev::EventSummary::AbsoluteAxis(_touchpad_event, AbsoluteAxisCode::ABS_X | AbsoluteAxisCode::ABS_Y, xy) = event.destructure() {
                    state.respond_motion(event.code() as usize, xy)
                }
            }
        };
    }
}

#[repr(u8)]
#[derive(PartialEq, Clone, Copy)]
enum Section {
    Global,
    Touchpad,
}

fn parse_config(config_path: &str) -> Result<Config, Error> {
    let mut config = Config::default();
    let mut section = Section::Global;
    let mut newline = 0;
    let config_string =
        fs::read_to_string(config_path).map_err(|io| Error::FailedReadingConfig {
            io,
            path: config_path.to_string(),
        })?;
    for line in config_string.trim().lines() {
        let line = line.trim();
        match line {
            "" => {
                if newline == 1 {
                    section = Section::Global;
                    newline = 0;
                }
                newline += 1;
                continue;
            }
            "[touchpad]" => {
                section = Section::Touchpad;
                newline = 0;
                continue;
            }
            _ => {
                newline = 0;
            }
        }

        let Some((key, value)) = line.split_once("=") else {
            Err(Error::InvalidConfig(line.to_owned()))?
        };

        match (section, key, value) {
            (Section::Global, "device", "autodetect") => {}
            (Section::Global, "device", device_path) => {
                config.keyboard_device = Some(device_path.to_owned())
            }
            (Section::Global, "modifiers", comma_separated_modifiers) => {
                for modifier_str in comma_separated_modifiers.split(",") {
                    let modifier = modifier_name_to_key_code(modifier_str)
                        .ok_or_else(|| Error::InvalidModifier(modifier_str.to_owned()))?;
                    config.modifiers.push(modifier);
                }
            }
            (Section::Global, "timeout", timeout_str) => match timeout_str.parse() {
                Ok(milliseconds) => config.timeout = milliseconds,
                Err(_) => Err(Error::InvalidTimeout(timeout_str.to_owned()))?,
            },
            (Section::Global, "clear_all_with_escape", value) => {
                config.clear_all_with_escape = yesnt(value, line)?
            }

            (Section::Touchpad, "timeout", timeout_str) => match timeout_str.parse() {
                Ok(milliseconds) => config.touchpad_timeout = milliseconds,
                Err(_) => Err(Error::InvalidTimeout(timeout_str.to_owned()))?,
            },
            (Section::Touchpad, "fuzz", fuzz_str) => match fuzz_str.parse() {
                Ok(milliseconds) => config.touchpad_fuzz = milliseconds,
                Err(_) => Err(Error::InvalidFuzz(fuzz_str.to_owned()))?,
            },
            (Section::Touchpad, "enabled", touchpad) => config.touchpad = yesnt(touchpad, line)?,
            _ => Err(Error::InvalidConfig(line.to_owned()))?,
        }
    }
    Ok(config)
}

fn yesnt(s: &str, line: &str) -> Result<bool, Error> {
    Ok(match s.to_lowercase().as_ref() {
        "yes" | "true" => true,
        "no" | "false" => false,
        _ => Err(Error::InvalidConfig(line.to_string()))?,
    })
}

fn modifier_name_to_key_code(s: &str) -> Option<KeyCode> {
    let ret = match s {
        "leftshift" => KeyCode::KEY_LEFTSHIFT,
        "rightshift" => KeyCode::KEY_RIGHTSHIFT,
        "leftctrl" => KeyCode::KEY_LEFTCTRL,
        "rightctrl" => KeyCode::KEY_RIGHTCTRL,
        "compose" => KeyCode::KEY_COMPOSE,
        "leftmeta" => KeyCode::KEY_LEFTMETA,
        "fn" => KeyCode::KEY_FN,
        "capslock" => KeyCode::KEY_CAPSLOCK,
        "rightmeta" => KeyCode::KEY_RIGHTMETA,
        _ => return None,
    };
    Some(ret)
}
