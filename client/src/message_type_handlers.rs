use std::error::Error;

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

pub fn handle_frame_full(payload: &[u8], pixels: &mut pixels::Pixels) -> Result<(), Box<dyn Error>> {
    //load img and convert to rgba
    // let img = image::load_from_memory(payload)?;
    // let rgba = img.to_rgba8();
    let width = u32::from_be_bytes(payload[0..4].try_into().unwrap()) as usize;
    let height = u32::from_be_bytes(payload[4..8].try_into().unwrap()) as usize;
    let data = &payload[8..];
   

    let extent = pixels.texture().size();
    let frame = pixels.frame_mut();
    // let win_width = extent.width;
    // let win_height = extent.height;

    if width != extent.width as usize || height != extent.height as usize {
        println!("Frame size {} {} does not match window size {} {}", width, height, extent.width, extent.height);
        return Ok(());
    }

    if data.len() != width * height * 4 {
        return Err("FrameFull pixel data length mismatch".into());
    }

    frame.copy_from_slice(data);
    Ok(())
}

pub fn handle_frame_delta(payload: &[u8], pixels: &mut pixels::Pixels) -> Result<(), Box<dyn Error>>  {
    if payload.len() < 16 {
        eprintln!("Frame delta too short");
        return Ok(());
    }

    //get payload metadata
    let x = u32::from_be_bytes(payload[0..4].try_into().unwrap()) as usize;
    let y = u32::from_be_bytes(payload[4..8].try_into().unwrap()) as usize;
    let w = u32::from_be_bytes(payload[8..12].try_into().unwrap()) as usize;
    let h = u32::from_be_bytes(payload[12..16].try_into().unwrap()) as usize;

    let expected_len = w * h * 4;
    if payload.len() < 16 + expected_len {
        eprintln!("Payload has wrong length");
        return Ok(());
    }

    let data = &payload[16..16 + expected_len];

    //get target frame buffer
    let extent = pixels.texture().size();
    let frame = pixels.frame_mut();
    let frame_w = extent.width as usize;
    let frame_h = extent.height as usize;

    if x + w > frame_w || y + h > frame_h {
        eprintln!("Frame delta out of bounds: rect {}x{} at ({}, {}) exceeds {}x{}", w, h, x, y, frame_w, frame_h);
        return Ok(());
    }

    for row in 0..h {
        let dest_start = ((y + row) * frame_w + x) * 4;
        let dest_end = dest_start + (w * 4);
        let src_start = row * w * 4;
        let src_end = src_start + (w * 4);

        frame[dest_start..dest_end].copy_from_slice(&data[src_start..src_end]);
    }

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
    println!("Key down: {:?}", payload);

    Ok(())
}

pub fn handle_key_up(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Key up: {:?}", payload);

    Ok(())
}

pub fn handle_mouse_move(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Mouse move: {:?}", payload);

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