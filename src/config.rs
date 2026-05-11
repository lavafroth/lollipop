use crate::Error;
use evdev::KeyCode;

#[repr(u8)]
#[derive(PartialEq, Clone, Copy)]
enum Section {
    Global,
    Touchpad,
}

pub struct Config {
    pub(crate) modifiers: Vec<KeyCode>,
    pub(crate) timeout: u64,
    pub(crate) keyboard_device: Option<String>,
    pub(crate) clear_all_with_escape: bool,
    pub(crate) touchpad: bool,
    pub(crate) touchpad_timeout: u64,
    pub(crate) touchpad_slop: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            touchpad_slop: 50,
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

impl Config {
    pub fn try_from_path(config_path: &str) -> Result<Config, Error> {
        let mut config = Config::default();
        let mut section = Section::Global;
        let mut newline = 0;
        let config_string =
            std::fs::read_to_string(config_path).map_err(|io| Error::FailedReadingConfig {
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
                (Section::Touchpad, "slop", slop_str) => match slop_str.parse() {
                    Ok(slop) => config.touchpad_slop = slop,
                    Err(_) => Err(Error::InvalidSlop(slop_str.to_owned()))?,
                },
                (Section::Touchpad, "enable", touchpad) => config.touchpad = yesnt(touchpad, line)?,
                _ => Err(Error::InvalidConfig(line.to_owned()))?,
            }
        }
        Ok(config)
    }
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
        "leftalt" => KeyCode::KEY_LEFTALT,
        _ => return None,
    };
    Some(ret)
}
pub fn key_code_to_modifier_name(s: KeyCode) -> Option<&'static str> {
    let ret = match s {
        KeyCode::KEY_LEFTSHIFT => "leftshift",
        KeyCode::KEY_RIGHTSHIFT => "rightshift",
        KeyCode::KEY_LEFTCTRL => "leftctrl",
        KeyCode::KEY_RIGHTCTRL => "rightctrl",
        KeyCode::KEY_COMPOSE => "compose",
        KeyCode::KEY_LEFTMETA => "leftmeta",
        KeyCode::KEY_FN => "fn",
        KeyCode::KEY_CAPSLOCK => "capslock",
        KeyCode::KEY_RIGHTMETA => "rightmeta",
        KeyCode::KEY_LEFTALT => "leftalt",
        _ => return None,
    };
    Some(ret)
}
