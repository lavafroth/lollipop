use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode, KeyEvent};
use std::thread::sleep;
use std::time::Duration;

pub enum KeyState {
    Latched(),
    Locked,
    None,
}

fn main() -> std::io::Result<()> {
    let mut keys = AttributeSet::<KeyCode>::default();
    keys.insert(KeyCode::KEY_A);

    let mut device = VirtualDevice::builder()?
        .name("lollipop")
        .with_keys(&keys)?
        .build()?;

    for path in device.enumerate_dev_nodes_blocking()? {
        let path = path?;
        println!("Available as {}", path.display());
    }

    sleep(Duration::from_secs(2));

    let code = KeyCode::KEY_A.code();

    loop {
        // this guarantees a key event
        let down_event = *KeyEvent::new(KeyCode(code), 1);
        device.emit(&[down_event]).unwrap();
        println!("Pressed.");
        sleep(Duration::from_secs(2));

        // alternativeley we can create a InputEvent, which will be any variant of InputEvent
        // depending on the type_ value
        let up_event = InputEvent::new(EventType::KEY.0, code, 0);
        device.emit(&[up_event]).unwrap();
        println!("Released.");
        sleep(Duration::from_secs(2));
    }

    Ok(())
}
