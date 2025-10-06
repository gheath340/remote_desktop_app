use std::{ 
    io::{ Read, Write, ErrorKind }, 
    error::Error, 
    process,
};
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

//helper function to get display capturer and rgba image from capturer
pub fn create_capturer_convert_to_rgba() -> Result<(usize, usize, Vec<u8>), Box<dyn Error>> {
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

    //get the actual width of each line including buffer
    let stride = frame.len() / height;
    let mut rgba = Vec::with_capacity(width * height * 4);

    for y in 0..height {
        //for each row, start at the beginning of the row
        let row_start = y * stride;
        //go from start of row to end of expected lenght, ignoring buffer
        let row_end = row_start + width * 4;
        let row = &frame[row_start..row_end];

        //for each 4 byte chunk in row get the value
        for chunk in row.chunks(4) {
            let b = chunk[0];
            let g = chunk[1];
            let r = chunk[2];
            let a = 255;
            //reorder them for rgba and add to rgba Vec
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    Ok((width, height, rgba))
}

pub fn handle_frame_full<T: Write>(stream: &mut T) -> Result<(), Box<dyn Error>> {
    //get image to display and dimensions
    let (width, height, rgba) = create_capturer_convert_to_rgba()?;

    //add image dimensions and image data to payload
    let mut payload = Vec::with_capacity(8 + rgba.len());
    payload.extend_from_slice(&(width as u32).to_be_bytes());
    payload.extend_from_slice(&(height as u32).to_be_bytes());
    payload.extend_from_slice(&rgba);
    
    send_response(stream, MessageType::FrameFull, &payload)?;
    Ok(())
}
pub fn handle_frame_delta<T: Write>(stream: &mut T, prev_frame: &mut Vec<u8>, width: usize, height: usize) -> Result<(), Box<dyn Error>> {
    //get screen as rgba
    let (_, _, rgba) = create_capturer_convert_to_rgba()?;

    //block size for delta comparison
    let block_size = 128;

    let mut frame_changes = Vec::new();
    let mut rect_count = 0u32;

        //calculate block width and height so edge blocks dont overflow
        //then compare current frame pixles vs previous frame, mark block as changed if different
        for by in (0..height).step_by(block_size) {
            for bx in (0..width).step_by(block_size) {
                let bw = block_size.min(width - bx);
                let bh = block_size.min(height - by);

                let mut changed = false;
                'outer: for row in 0..bh {
                    let cur_off = ((by + row) * width + bx) * 4;
                    let prev_off = cur_off;
                    let len = bw * 4;
                    if &rgba[cur_off..cur_off + len] != &prev_frame[prev_off..prev_off + len] {
                        changed = true;
                        break 'outer;
                    }
                }
                //if block changed build payload with new info and send to client
                if changed {
                    rect_count += 1;
                    // let mut rect_bytes = Vec::with_capacity(16 + bw * bh * 4);
                    frame_changes.extend_from_slice(&(bx as u32).to_be_bytes());
                    frame_changes.extend_from_slice(&(by as u32).to_be_bytes());
                    frame_changes.extend_from_slice(&(bw as u32).to_be_bytes());
                    frame_changes.extend_from_slice(&(bh as u32).to_be_bytes());

                    for row in 0..bh {
                        let start = ((by + row) * width + bx) * 4;
                        let end = start + bw * 4;
                        // rect_bytes.extend_from_slice(&rgba[start..end]);
                        frame_changes.extend_from_slice(&rgba[start..end]);
                    }

                    //send_response(stream, MessageType::FrameDelta, &rect_bytes)?;
                }
            }
        }
        if rect_count > 0 {
            let mut payload = Vec::with_capacity(4 + frame_changes.len());
            payload.extend_from_slice(&rect_count.to_be_bytes());
            payload.extend_from_slice(&frame_changes);

            send_response(stream, MessageType::FrameDelta, &payload)?;
        }
        send_response(stream, MessageType::FrameEnd, &[])?;

        // Save this frame for next delta comparison
        *prev_frame = rgba;
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