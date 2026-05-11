use evdev::{AbsoluteAxisCode, Device, EventStream, InputEvent, KeyEvent, LedCode, LedEvent};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io;
use std::time::{Duration, SystemTime};
mod config;
mod touchpad;
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

pub struct InternalState {
    modifiers: BTreeMap<KeyCode, KeyState>,
    timeout: Duration,
    clear_all_with_escape: bool,
    touchpad: touchpad::Touchpad,
}
impl InternalState {
    fn release_latched(&mut self) -> Vec<InputEvent> {
        let mut events = vec![];
        for (key, key_state) in self.modifiers.iter_mut() {
            if let KeyState::Latched(_) = key_state {
                *key_state = KeyState::None;
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
        state.modifiers.insert(key, KeyState::None);
    }

    let mut keyboard_events = keyboard.into_event_stream()?;

    loop {
        tokio::select! {
            _ = state.touchpad.timeout() => {
                lollipop_virtual_device.emit(&state.release_latched())?;
                led_sink.send_events(&[*LedEvent::new(LedCode::LED_CAPSL, state.led_state())])?;
            }

            Ok(event) = keyboard_events.next_event() => {
                if let evdev::EventSummary::Key(key_event, key_code, pressed) = event.destructure() {
                    let events = state.transition(key_code, pressed, key_event.timestamp());
                    // println!("{state:#?}");
                    lollipop_virtual_device.emit(&events)?;
                    led_sink.send_events(&[*LedEvent::new(LedCode::LED_CAPSL, state.led_state())])?;
                }
            }

            Some(Ok(event)) = handle_touchpad(touchpad_events.as_mut()) => {

                if let evdev::EventSummary::Key(_key_event,
                    KeyCode::BTN_LEFT | KeyCode::BTN_RIGHT | KeyCode::BTN_TOUCH, pressed) = event.destructure() {
                    state.touchpad.respond_touch(pressed);
                    led_sink.send_events(&[*LedEvent::new(LedCode::LED_CAPSL, state.led_state())])?;
                }
                if let evdev::EventSummary::AbsoluteAxis(_touchpad_event,
                    AbsoluteAxisCode::ABS_X | AbsoluteAxisCode::ABS_Y, xy) = event.destructure() {
                    state.touchpad.respond_motion(event.code() as usize, xy)
                }
            }
        }
    }
}
