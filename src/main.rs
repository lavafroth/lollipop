use evdev::{AbsoluteAxisCode, Device, InputEvent, KeyEvent, LedCode, LedEvent};
use nix::sys::epoll;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs;
use std::time::{Duration, SystemTime};

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
    tap_held: bool,
    position: [i32; 2],
    buffer: Vec<InputEvent>,
    last_release: Option<SystemTime>,
}

pub struct InternalState {
    modifiers: BTreeMap<KeyCode, KeyState>,
    timeout: Duration,
    clear_all_with_escape: bool,
    touchpad: Touchpad,
}

const TOUCH_RELEASED: i32 = 0;

impl InternalState {
    fn respond_touch(&mut self, this_keypress: i32) {
        if this_keypress == 1 {
            // println!("tapped");
            self.touchpad.tap_held = true;
            self.touchpad.last_release = None;
        }

        if self.touchpad.tap_held && this_keypress == TOUCH_RELEASED {
            self.touchpad.last_release = Some(SystemTime::now());

            for (key, key_state) in self.modifiers.iter_mut() {
                if !KeyState::None.eq(key_state) {
                    *key_state = KeyState::None;
                    self.touchpad.buffer.push(*KeyEvent::new(*key, 0));
                }
            }
        }
    }
    fn touchpad_dragging(&mut self, index: usize, xy: i32) {
        if !self.touchpad.tap_held {
            return;
        }

        if self.touchpad.position[index] == -1 {
            self.touchpad.position[index] = xy;
            return;
        }

        if (self.touchpad.position[index] - xy).abs() > 300 {
            self.touchpad.tap_held = false;
            self.touchpad.position = [-1, -1];
        }
    }
    fn transition(&mut self, key: KeyCode, pressed: i32, timestamp: SystemTime) -> Vec<InputEvent> {
        if self.clear_all_with_escape && key == KeyCode::KEY_ESC {
            let mut events = vec![];
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

        let mut events = vec![*KeyEvent::new(key, pressed)];
        for (key, key_state) in self.modifiers.iter_mut() {
            if let KeyState::Latched(_) = key_state {
                *key_state = KeyState::None;
                events.push(*KeyEvent::new(*key, 0));
            }
        }

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
}

impl Default for Config {
    fn default() -> Self {
        Self {
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

fn main() -> Result<(), anyhow::Error> {
    let config = match std::env::args().nth(1) {
        Some(config_file) => parse_config(&config_file)?,
        None => Config::default(),
    };

    let epoll = epoll::Epoll::new(epoll::EpollCreateFlags::EPOLL_CLOEXEC)?;
    let event = epoll::EpollEvent::new(epoll::EpollFlags::EPOLLIN, 0);

    let (mut keyboard, mut led_sink) = if let Some(device_path) = config.keyboard_device {
        open_device(&device_path)?
    } else {
        (pick_device()?, pick_device()?)
    };

    keyboard.set_nonblocking(true)?;
    epoll.add(&keyboard, event)?;

    let mut touchpad = if config.touchpad {
        let touchpad = pick_touchpad()?;
        touchpad.set_nonblocking(true)?;
        let touchpad_epoll_event = epoll::EpollEvent::new(epoll::EpollFlags::EPOLLIN, 0);
        epoll.add(&touchpad, touchpad_epoll_event)?;
        Some(touchpad)
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
            tap_held: false,
            position: [-1, -1],
            buffer: vec![],
            last_release: None,
        },
    };

    for key in config.modifiers {
        state.modifiers.insert(key, KeyState::None);
    }

    loop {
        if state
            .touchpad
            .last_release
            .and_then(|v| v.elapsed().ok())
            .is_some_and(|v| v > Duration::from_millis(200))
        {
            lollipop_virtual_device.emit(&state.touchpad.buffer)?;
            state.touchpad.buffer.clear();
        }
        match keyboard.fetch_events() {
            Ok(iterator) => {
                for event in iterator {
                    if let evdev::EventSummary::Key(key_event, key_code, pressed) =
                        event.destructure()
                    {
                        let events = state.transition(key_code, pressed, key_event.timestamp());
                        // println!("{state:#?}");
                        lollipop_virtual_device.emit(&events)?;
                        led_sink.send_events(&[*LedEvent::new(
                            LedCode::LED_CAPSL,
                            state.led_state(),
                        )])?;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => {
                eprintln!("{e}");
                // break;
            }
        }
        if let Some(touchpad) = touchpad.as_mut() {
            match touchpad.fetch_events() {
                Ok(iterator) => {
                    for event in iterator {
                        if let evdev::EventSummary::Key(
                            _key_event,
                            KeyCode::BTN_LEFT | KeyCode::BTN_RIGHT | KeyCode::BTN_TOUCH,
                            pressed,
                        ) = event.destructure()
                        {
                            state.respond_touch(pressed);
                            led_sink.send_events(&[*LedEvent::new(
                                LedCode::LED_CAPSL,
                                state.led_state(),
                            )])?;
                        }
                        if let evdev::EventSummary::AbsoluteAxis(
                            _touchpad_event,
                            AbsoluteAxisCode::ABS_X | AbsoluteAxisCode::ABS_Y,
                            xy,
                        ) = event.destructure()
                        {
                            state.touchpad_dragging(event.code() as usize, xy)
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    eprintln!("{e}");
                    // break;
                }
            }
        }
    }
}

fn parse_config(config_path: &str) -> Result<Config, Error> {
    let mut config = Config::default();
    let config_string =
        fs::read_to_string(config_path).map_err(|io| Error::FailedReadingConfig {
            io,
            path: config_path.to_string(),
        })?;
    for line in config_string.trim().lines() {
        match line.split_once("=") {
            Some(("modifiers", comma_separated_modifiers)) => {
                for modifier_str in comma_separated_modifiers.split(",") {
                    let modifier = modifier_name_to_key_code(modifier_str)
                        .ok_or_else(|| Error::InvalidModifier(modifier_str.to_owned()))?;
                    config.modifiers.push(modifier);
                }
            }
            Some(("timeout", timeout_str)) => match timeout_str.parse() {
                Ok(milliseconds) => config.timeout = milliseconds,
                Err(_) => Err(Error::InvalidTimeout(timeout_str.to_owned()))?,
            },
            Some(("device", "autodetect")) => {}
            Some(("clear_all_with_escape", clear_all_with_escape)) => {
                match clear_all_with_escape.to_lowercase().as_ref() {
                    "yes" | "true" => config.clear_all_with_escape = true,
                    "no" | "false" => config.clear_all_with_escape = false,
                    _ => Err(Error::InvalidConfig(line.to_string()))?,
                }
            }
            Some(("touchpad", touchpad)) => match touchpad.to_lowercase().as_ref() {
                "yes" | "true" => config.touchpad = true,
                "no" | "false" => config.touchpad = false,
                _ => Err(Error::InvalidConfig(line.to_string()))?,
            },
            Some(("device", device_path)) => config.keyboard_device = Some(device_path.to_owned()),
            _ => Err(Error::InvalidConfig(line.to_owned()))?,
        }
    }
    Ok(config)
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
