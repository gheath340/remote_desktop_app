use std::{
    error::Error,
    time::Instant,
    process::{ Command, },
};
use common::message_type::MessageType;

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

pub fn handle_key_down(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
   if payload.len() < 10 {
        return Err("KeyDown payload too short".into());
    }

    let scancode = u32::from_be_bytes(payload[0..4].try_into()?);
    let modifiers = u32::from_be_bytes(payload[4..8].try_into()?);

    // vk_name optional
    let vk_name_len = u16::from_be_bytes(payload[8..10].try_into()?);
    let vk_name = if vk_name_len > 0 && payload.len() >= 10 + vk_name_len as usize {
        Some(std::str::from_utf8(&payload[10..10 + vk_name_len as usize])?)
    } else {
        None
    };

    // ydotool key press: send scancode
    Command::new("ydotool")
        .arg("key")
        .arg(scancode.to_string())
        .status()?;

    Ok(())
}

pub fn handle_key_up(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    if payload.len() < 10 {
        return Err("KeyUp payload too short".into());
    }

    let scancode = u32::from_be_bytes(payload[0..4].try_into()?);

    // ydotool key up: note ydotool defaults to key press, for key up you might need 'keyup' depending on setup
    Command::new("ydotool")
        .arg("keyup")
        .arg(scancode.to_string())
        .status()?;

    Ok(())
}

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