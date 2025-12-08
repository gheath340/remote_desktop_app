use std::{
    error::Error,
    time::Instant,
    process::Command,
};
use common::message_type::MessageType;

#[derive(Debug, Default)]
pub struct ModifiersState {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool, // Cmd/Meta
}

pub fn handle_text(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Text message: {:?}", String::from_utf8_lossy(payload));

    Ok(())
}

pub fn handle_connect(_payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Client connected (connect message)");

    Ok(())
}

pub fn handle_disconnect(_payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Client requested disconnect");

    Ok(())
}

pub fn handle_error(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Error message: {:?}", String::from_utf8_lossy(payload));

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn handle_mouse_move(payload: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

   if payload.len() < 8 {
        return Err("Invalid MouseMove payload".into());
    }

    // Parse x/y from big-endian u32
    let x = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let y = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
    println!("Mouse move: x={}, y={}", x, y);

    let src = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Failed to create CGEventSource")?;

    //create event
    let pos = CGPoint::new(x as f64, y as f64);
    let move_event = CGEvent::new_mouse_event(src, CGEventType::MouseMoved, pos, CGMouseButton::Left)
        .map_err(|_| "Failed to create CGEvent")?;

    //post it to the system
    move_event.post(CGEventTapLocation::HID);

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn handle_mouse_move(payload: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    if payload.len() < 8 {
        return Err("Invalid MouseMove payload".into());
    }

    // Parse x/y from big-endian u32
    let x = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let y = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
    println!("Mouse move: x={}, y={}", x, y);

    //Call ydotool to actually move the cursor
    std::process::Command::new("ydotool")
        .arg("mousemove")
        .arg(x.to_string())
        .arg(y.to_string())
        .status()?;

    Ok(())
}

pub fn handle_cursor_shape(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Cursor shape update: {} bytes", payload.len());

    Ok(())
}

pub fn handle_cursor_pos(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    if payload.len() == 8 {
        let x = u32::from_be_bytes(payload[0..4].try_into().unwrap());
        let y = u32::from_be_bytes(payload[4..8].try_into().unwrap());
        println!("Cursor moved to ({x}, {y})");
    } else {
        println!("Invalid cursor pos payload");
    }

    Ok(())
}

pub fn handle_resize(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    if payload.len() == 8 {
        let w = u32::from_be_bytes(payload[0..4].try_into().unwrap());
        let h = u32::from_be_bytes(payload[4..8].try_into().unwrap());
        println!("Resize request: {w}x{h}");
    } else {
        println!("Invalid resize payload");
    }

    Ok(())
}

fn mac_to_linux_scancode(mac: u32) -> Option<u32> {
    match mac {
        // Letters
        0  => Some(30), // A
        11 => Some(48), // B
        8  => Some(46), // C
        2  => Some(32), // D
        14 => Some(18), // E
        3  => Some(33), // F
        5  => Some(34), // G
        4  => Some(35), // H
        34 => Some(23), // I
        38 => Some(36), // J
        40 => Some(37), // K
        37 => Some(38), // L
        46 => Some(50), // M
        45 => Some(49), // N
        31 => Some(24), // O
        35 => Some(25), // P
        12 => Some(16), // Q
        15 => Some(19), // R
        1  => Some(31), // S
        17 => Some(20), // T
        32 => Some(22), // U
        9  => Some(47), // V
        13 => Some(17), // W
        7  => Some(45), // X
        16 => Some(21), // Y
        6  => Some(44), // Z

        // Numbers row
        18 => Some(2),  // 1
        19 => Some(3),  // 2
        20 => Some(4),  // 3
        21 => Some(5),  // 4
        23 => Some(6),  // 5
        22 => Some(7),  // 6
        26 => Some(8),  // 7
        28 => Some(9),  // 8
        25 => Some(10), // 9
        29 => Some(11), // 0

        // Punctuation / symbols
        27 => Some(12), // -
        24 => Some(13), // =
        33 => Some(26), // [
        30 => Some(27), // ]
        41 => Some(39), // ;
        39 => Some(40), // '
        43 => Some(51), // ,
        47 => Some(52), // .
        44 => Some(53), // /
        42 => Some(43), // \

        // Whitespace & control
        49 => Some(57),  // Space
        36 => Some(28),  // Enter / Return
        48 => Some(15),  // Tab
        51 => Some(14),  // Backspace
        53 => Some(1),   // Escape
        57 => Some(58),  // CapsLock

        // Modifiers
        56 => Some(42),  // Left Shift
        60 => Some(54),  // Right Shift
        59 => Some(29),  // Left Ctrl
        62 => Some(97),  // Right Ctrl
        58 => Some(56),  // Left Alt
        61 => Some(100), // Right Alt (AltGr)
        55 => Some(125), // Left Cmd / Meta
        63 => Some(126), // Right Cmd / Meta

        // Arrow keys
        123 => Some(105), // Left
        124 => Some(106), // Right
        125 => Some(108), // Down
        126 => Some(103), // Up

        _ => None, // Unknown / unsupported keys
    }
}

/// Convert a 32-bit integer representing modifier bits into a ModifiersState
pub fn bits_to_modifiers(bits: u32) -> ModifiersState {
    ModifiersState {
        shift: (bits & 0b0001) != 0,
        ctrl:  (bits & 0b0010) != 0,
        alt:   (bits & 0b0100) != 0,
        logo:  (bits & 0b1000) != 0, // Cmd / Meta
    }
}

pub fn handle_key(msg_type: MessageType, payload: &[u8]) -> Result<(), Box<dyn Error>> {
    if payload.len() < 8 {
        return Err("Key payload too short".into());
    }

    // Parse payload
    let modifier_bits = u32::from_be_bytes(payload[0..4].try_into()?);
    let mac_scancode = u32::from_be_bytes(payload[4..8].try_into()?);

    if mac_scancode == 0 {
        return Ok(()); // Ignore invalid scancodes
    }

    let linux_scancode = match mac_to_linux_scancode(mac_scancode) {
        Some(code) => code,
        None => return Ok(()), // Unknown key
    };

    let down = matches!(msg_type, MessageType::KeyDown);

    // Convert modifier bits to ModifiersState (you should have this function implemented)
    let modifiers = bits_to_modifiers(modifier_bits);

    // Handle Shift for capital letters and symbols
    let mut needs_shift = false;

    // If Shift is pressed on client, or CapsLock is active for letters
    if modifiers.shift {
        needs_shift = true;
    }

    // Press Shift if needed
    if needs_shift && down {
        Command::new("ydotool")
            .arg("key")
            .arg("42") // Left Shift
            .status()?;
    }

    // Press or release the actual key
    let action = if down { "key" } else { "keyup" };
    Command::new("ydotool")
        .arg(action)
        .arg(linux_scancode.to_string())
        .status()?;

    // Release Shift if needed
    if needs_shift && !down {
        Command::new("ydotool")
            .arg("keyup")
            .arg("42") // Left Shift
            .status()?;
    }

    Ok(())
}

// pub fn handle_key_up(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
//     if payload.len() < 10 {
//         return Err("KeyUp payload too short".into());
//     }

//     let scancode = u32::from_be_bytes(payload[0..4].try_into()?);

//     // ydotool key up: note ydotool defaults to key press, for key up you might need 'keyup' depending on setup
//     Command::new("ydotool")
//         .arg("keyup")
//         .arg(scancode.to_string())
//         .status()?;

//     Ok(())
// }

pub fn handle_mouse_down(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Mouse down: {:?}", payload);

    Ok(())
}

pub fn handle_mouse_up(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Mouse up: {:?}", payload);

    Ok(())
}

pub fn handle_mouse_scroll(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Mouse scroll: {:?}", payload);

    Ok(())
}

pub fn handle_clipboard(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Clipboard data: {:?}", String::from_utf8_lossy(payload));

    Ok(())
}