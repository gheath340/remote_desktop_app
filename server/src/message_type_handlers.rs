use std::error::Error;
use std::io::{ Read, Write, ErrorKind };
use crate::tcp_server::send_response;
use common::message_type::MessageType;
use image::codecs::png::PngEncoder;
use image::ImageEncoder;

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

pub fn handle_frame_full<T: Write>(stream: &mut T) -> Result<(), Box<dyn Error>> {
    //get display(main monitor) and capturer(captures display)
    let display = scrap::Display::primary()?;
    let mut capturer = scrap::Capturer::new(display)?;

    let width = capturer.width();
    let height = capturer.height();

    //grab frame, if WouldBlock wait and try again until it works
    let frame = loop {
        match capturer.frame() {
            Ok(frame) => break frame,
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(5));
                continue;
            }
            Err(e) => return Err(Box::new(e))
        }
    };

    //scrap gives BGRA, image crate needs RGBA, change to RGBA
    let mut rgba = Vec::with_capacity(width * height * 4);
    for chunk in frame.chunks(4) {
        let b = chunk[0];
        let g = chunk[1];
        let r = chunk[2];
        let a = 255;
        rgba.extend_from_slice(&[r, g, b, a]);
    }

    //put into ImageBuffer(rust image object)
    let img_buffer = image::RgbaImage::from_raw(width as u32, height as u32, rgba).
        ok_or("Failed to create image buffer")?;

    //encode as PNG
    let mut png_bytes = Vec::new();
    PngEncoder::new(&mut png_bytes)
        .write_image(
            &img_buffer,
            width as u32,
            height as u32,
            image::ColorType::Rgba8,
        )?;
    
    send_response(stream, MessageType::FrameFull, &png_bytes)?;
    Ok(())
}

pub fn handle_frame_delta(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Frame delta received: {} bytes", payload.len());

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