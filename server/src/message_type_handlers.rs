

pub fn handle_text(payload: &[u8]) {
    println!("Text message: {:?}", String::from_utf8_lossy(payload));
}

pub fn handle_connect(_payload: &[u8]) {
    println!("Client connected (connect message)");
}

pub fn handle_disconnect(_payload: &[u8]) {
    println!("Client requested disconnect");
}

pub fn handle_error(payload: &[u8]) {
    println!("Error message: {:?}", String::from_utf8_lossy(payload));
}

pub fn handle_frame_full(payload: &[u8]) {
    println!("Full frame received: {} bytes", payload.len());
}

pub fn handle_frame_delta(payload: &[u8]) {
    println!("Frame delta received: {} bytes", payload.len());
}

pub fn handle_cursor_shape(payload: &[u8]) {
    println!("Cursor shape update: {} bytes", payload.len());
}

pub fn handle_cursor_pos(payload: &[u8]) {
    if payload.len() == 8 {
        let x = u32::from_be_bytes(payload[0..4].try_into().unwrap());
        let y = u32::from_be_bytes(payload[4..8].try_into().unwrap());
        println!("Cursor moved to ({x}, {y})");
    } else {
        println!("Invalid cursor pos payload");
    }
}

pub fn handle_resize(payload: &[u8]) {
    if payload.len() == 8 {
        let w = u32::from_be_bytes(payload[0..4].try_into().unwrap());
        let h = u32::from_be_bytes(payload[4..8].try_into().unwrap());
        println!("Resize request: {w}x{h}");
    } else {
        println!("Invalid resize payload");
    }
}

pub fn handle_key_down(payload: &[u8]) {
    println!("Key down: {:?}", payload);
}

pub fn handle_key_up(payload: &[u8]) {
    println!("Key up: {:?}", payload);
}

pub fn handle_mouse_move(payload: &[u8]) {
    println!("Mouse move: {:?}", payload);
}

pub fn handle_mouse_down(payload: &[u8]) {
    println!("Mouse down: {:?}", payload);
}

pub fn handle_mouse_up(payload: &[u8]) {
    println!("Mouse up: {:?}", payload);
}

pub fn handle_mouse_scroll(payload: &[u8]) {
    println!("Mouse scroll: {:?}", payload);
}

pub fn handle_clipboard(payload: &[u8]) {
    println!("Clipboard data: {:?}", String::from_utf8_lossy(payload));
}