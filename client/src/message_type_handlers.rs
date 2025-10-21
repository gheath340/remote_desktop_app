use std::error::Error;
use image::{ImageBuffer, Rgba, imageops::resize, imageops::FilterType};


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
    //get the current display area size
    let extent = pixels.texture().size();

    //build ImageBuffer from the raw RGBA bytes
    let img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, payload.to_vec())
        .ok_or("Invalid frame buffer")?;

    //scale it to fit the actual display surface
    let scaled = resize(&img, extent.width, extent.height, FilterType::Triangle);

    //write directly into pixel buffer
    let frame = pixels.frame_mut();
    frame.copy_from_slice(&scaled);

    Ok(())
}

pub fn handle_frame_delta(payload: &[u8], pixels: &mut pixels::Pixels) -> Result<(), Box<dyn Error>>  {
    //if payload is too short error out
    if payload.len() < 16 {
        eprintln!("Frame delta too short");
        return Ok(());
    }

    //first 4 bytes are the number of changed rectangles
    let rect_count = u32::from_be_bytes(payload[0..4].try_into().unwrap()) as usize;
    //offset to start reading rectangles from correct position
    let mut offset = 4;

    //struct with frame dimensions
    let extent = pixels.texture().size();
    //mutable reference to pixel buffer
    let frame = pixels.frame_mut();
    //frame width
    let fw = extent.width as usize;

    for _ in 0..rect_count {
        //if there isn't enough data for the rectangle data error out
        if offset + 16 > payload.len() {
            return Err("Truncated FrameDelta payload".into());
        }

        //read rectangle metadata and update offset
        let x = u32::from_be_bytes(payload[offset..offset+4].try_into().unwrap()) as usize;
        let y = u32::from_be_bytes(payload[offset+4..offset+8].try_into().unwrap()) as usize;
        let w = u32::from_be_bytes(payload[offset+8..offset+12].try_into().unwrap()) as usize;
        let h = u32::from_be_bytes(payload[offset+12..offset+16].try_into().unwrap()) as usize;
        offset += 16;

        //calculate size of rectangle pixel data and ensure enough data is present
        let rect_size = w * h * 4;
        if offset + rect_size > payload.len() {
            return Err("Truncated FrameDelta pixel data".into());
        }

        //get rectangle pixel data and update offset
        let data = &payload[offset..offset+rect_size];
        offset += rect_size;

        //copy rectangle data into correct position in frame buffer
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