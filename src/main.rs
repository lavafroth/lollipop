use evdev::{Device, InputEvent, KeyEvent, LedCode, LedEvent};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs;
use std::time::{Duration, SystemTime};

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, KeyCode};
mod key_codes;

#[derive(Clone, Copy)]
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
}

impl InternalState {
    fn transition(&mut self, key: KeyCode, pressed: i32, timestamp: SystemTime) -> Vec<InputEvent> {
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

fn pick_device() -> Option<Device> {
    evdev::enumerate()
        .map(|(_, device)| device)
        .find(|d| d.name().is_some_and(|name| name.contains("keyboard")))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut modifiers = vec![];
    let mut timeout = 500;
    if let Some(config) = std::env::args().nth(1) {
        for line in fs::read_to_string(config)?.trim().lines() {
            match line.split_once("=") {
                Some(("modifiers", comma_separated_modifiers)) => {
                    for modifier in comma_separated_modifiers.split(",") {
                        match modifier_name_to_key_code(modifier) {
                            Some(modifier) => modifiers.push(modifier),
                            None => {
                                eprintln!(
                                    "invalid modifier `{modifier}` supplied in config, valid modifiers are: leftshift, rightshift, leftctrl, rightctrl, compose, leftmeta, fn, capslock, rightmeta"
                                );
                                std::process::exit(1);
                            }
                        }
                    }
                }
                Some(("timeout", timeout_str)) => match timeout_str.parse() {
                    Ok(milliseconds) => timeout = milliseconds,
                    Err(e) => {
                        eprintln!(
                            "failed to parse locking timeout from config: {e}, `{timeout_str}` supplied"
                        );
                        std::process::exit(1);
                    }
                },
                _ => {
                    eprintln!("invalid line in config: `{line}`");
                    std::process::exit(1);
                }
            }
        }
    }

    let (Some(mut keyboard), Some(mut led_sink)) = (pick_device(), pick_device()) else {
        return Ok(());
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
        modifiers: BTreeMap::default(),
        timeout: Duration::from_millis(timeout),
    };

    if modifiers.is_empty() {
        modifiers.extend([
            KeyCode::KEY_LEFTSHIFT,
            KeyCode::KEY_LEFTMETA,
            KeyCode::KEY_LEFTCTRL,
            KeyCode::KEY_LEFTALT,
        ]);
    }

    for key in modifiers {
        state.modifiers.insert(key, KeyState::None);
    }

    loop {
        // fetch events blocks for new events
        let Ok(events) = keyboard.fetch_events() else {
            continue;
        };
        for event in events {
            if let evdev::EventSummary::Key(key_event, key_code, pressed) = event.destructure() {
                let events = state.transition(key_code, pressed, key_event.timestamp());
                // println!("{state:#?}");
                lollipop_virtual_device.emit(&events)?;
                led_sink.send_events(&[*LedEvent::new(LedCode::LED_CAPSL, state.led_state())])?;
            }
        }
    }
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
