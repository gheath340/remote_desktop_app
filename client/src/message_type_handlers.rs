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

pub fn handle_frame_full(width: u32, height: u32, payload: &[u8], pixels: &mut pixels::Pixels) -> Result<(), Box<dyn Error>> {
    //get the window dimensions and frame
    let extent = pixels.texture().size();
    let frame = pixels.frame_mut();

    if width != extent.width || height != extent.height {
        println!("Frame size {} {} does not match window size {} {}", width, height, extent.width, extent.height);
        return Ok(());
    }

    if payload.len() != (width as usize) * (height as usize) * 4 {
        return Err("FrameFull pixel data length mismatch".into());
    }

    //add image data to frame to display image
    frame.copy_from_slice(payload);
    Ok(())
}

pub fn handle_frame_delta(payload: &[u8], pixels: &mut pixels::Pixels) -> Result<(), Box<dyn Error>>  {
    if payload.len() < 16 {
        eprintln!("Frame delta too short");
        return Ok(());
    }

    let rect_count = u32::from_be_bytes(payload[0..4].try_into().unwrap()) as usize;
    let mut offset = 4;

    let extent = pixels.texture().size();
    let frame = pixels.frame_mut();
    let fw = extent.width as usize;

    for _ in 0..rect_count {
        if offset + 16 > payload.len() {
            return Err("Truncated FrameDelta payload".into());
        }

        let x = u32::from_be_bytes(payload[offset..offset+4].try_into().unwrap()) as usize;
        let y = u32::from_be_bytes(payload[offset+4..offset+8].try_into().unwrap()) as usize;
        let w = u32::from_be_bytes(payload[offset+8..offset+12].try_into().unwrap()) as usize;
        let h = u32::from_be_bytes(payload[offset+12..offset+16].try_into().unwrap()) as usize;
        offset += 16;

        let rect_size = w * h * 4;
        if offset + rect_size > payload.len() {
            return Err("Truncated FrameDelta pixel data".into());
        }

        let data = &payload[offset..offset+rect_size];
        offset += rect_size;

        for row in 0..h {
            let dest_start = ((y + row) * fw + x) * 4;
            let dest_end = dest_start + w * 4;
            let src_start = row * w * 4;
            let src_end = src_start + w * 4;
            frame[dest_start..dest_end].copy_from_slice(&data[src_start..src_end]);
        }
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