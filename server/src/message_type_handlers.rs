use std::{
    io::{ Write },
    error::Error,
    time::Instant,
    process::{ Command, },
};
use crate::tcp_server::send_response;
use common::message_type::MessageType;
// use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
// use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
// use core_graphics::geometry::CGPoint;
use turbojpeg::{Compressor, Image, PixelFormat, Subsamp, OutputBuf};

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

pub fn handle_frame_full(compressor: &mut Compressor, output: &mut OutputBuf, rgba: &Vec<u8>, width: usize, height: usize) -> Result<(MessageType, Vec<u8>), Box<dyn Error>> {
    let t2 = Instant::now();

    //create image and tell decoder how to handle image
    let image = Image {
                pixels: rgba.as_slice(), //mut slice pointing to rgba buffer
                width: width, //width of jpeg
                pitch: width * 4, //how many bytes per row(width * 4 for rgba)
                height: height, //height of jpeg
                format: PixelFormat::RGBA, //the format you want the output to be
            };

    // Compress image into output buffer
    compressor.compress(image, output)?;

    //send reference of output buffer
    let full_frame_ms = t2.elapsed().as_millis();
    print!("Full frame: {}ms    ", full_frame_ms);
    let output_clone = output.as_ref().to_vec();

    Ok((MessageType::FrameFull, output_clone))
}

pub fn handle_frame_delta(prev_frame: &mut Vec<u8>, width: usize, height: usize, rgba: Vec<u8>) -> Result<(MessageType, Vec<u8>), Box<dyn Error>> {
    //start timer
    let start_total = Instant::now();

    // Create compressor + output buffer
    let mut compressor = Compressor::new()?;
    let _ = compressor.set_subsamp(Subsamp::Sub2x2);
    let _ = compressor.set_quality(80);
    let mut output = OutputBuf::new_owned();

    //get all frame changes, count of parts of screen that changed and amount of changed pixels
    let (frame_changes, rect_count, changed_pixels) = calculate_frame_changes(prev_frame, width, height, &rgba);

    //if there actually was a change
    if rect_count > 0 {
        //calculate how much of the image changed
        let total_pixels = width * height;
        let change_ratio = changed_pixels as f32 / total_pixels as f32;

        //if more than half the image changed handle it as a full frame change
        if change_ratio > 0.5 {
            let (msg_type, payload) = handle_frame_full(&mut compressor, &mut output, &rgba, width, height)?;
            let total_ms = start_total.elapsed().as_millis();
            println!("Handle frame otal: {}ms", total_ms);
            *prev_frame = rgba;
            return Ok((msg_type, payload));
        //if less than half of the image changed handle it as delta change
        } else {
            let t3 = Instant::now();
            let mut payload = Vec::with_capacity(4 + frame_changes.len());
            payload.extend_from_slice(&rect_count.to_be_bytes());
            payload.extend_from_slice(&frame_changes);

            let compressed = lz4_flex::compress_prepend_size(&payload);
            let delta_frame_ms = t3.elapsed().as_millis();
            print!("Delta frame: {}ms   ", delta_frame_ms);
            let total_ms = start_total.elapsed().as_millis();
            println!("Handle frame total: {}ms", total_ms);
            *prev_frame = rgba;
            return Ok((MessageType::FrameDelta, compressed));
        }
    }

    // Save this frame for next delta comparison
    *prev_frame = rgba;
    Ok((MessageType::FrameEnd, Vec::new()))
}

//calculate how many pixel blocks have changed
pub fn calculate_frame_changes(prev_frame: &mut Vec<u8>, width: usize, height: usize, rgba: &Vec<u8>) -> (Vec<u8>, u32, usize) {
    //size of block that will be checked each loop
    let block_size = 128;
    let mut changed_pixels: usize = 0;
    let mut frame_changes = Vec::new();
    let mut rect_count = 0u32;

    //loop through all blocks of block size in image and set height to either block size or less if at an edge
    for by in (0..height).step_by(block_size) {
        for bx in (0..width).step_by(block_size) {
            let bw = block_size.min(width - bx);
            let bh = block_size.min(height - by);

            //if pixels in this block are different from the same block in the last image mark as changed
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
            //if block changed add block position, block size, and block data to frame_changes
            if changed {
                changed_pixels += bw * bh;
                rect_count += 1;
                frame_changes.extend_from_slice(&(bx as u32).to_be_bytes());
                frame_changes.extend_from_slice(&(by as u32).to_be_bytes());
                frame_changes.extend_from_slice(&(bw as u32).to_be_bytes());
                frame_changes.extend_from_slice(&(bh as u32).to_be_bytes());

                for row in 0..bh {
                    let start = ((by + row) * width + bx) * 4;
                    let end = start + bw * 4;
                    frame_changes.extend_from_slice(&rgba[start..end]);
                }
            }
        }
    }
    //return changed frames, how many blocks changed and how many pixels changed
    (frame_changes, rect_count, changed_pixels)
}

//NEED TO MAKE THIS MORE EFFIECIENT, DONT WANT TO SPAWN NEW YDOTOOL PROCESS EVERY SINGLE MOUSE MOVEMENT
pub fn handle_mouse_move(payload: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
   if payload.len() < 8 {
        return Err("Invalid MouseMove payload".into());
    }

    // Parse x/y from big-endian u32
    let x = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let y = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
    println!("Mouse move: x={}, y={}", x, y);

    // let src = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
    //     .map_err(|_| "Failed to create CGEventSource")?;

    // //create event
    // let pos = CGPoint::new(x as f64, y as f64);
    // let move_event = CGEvent::new_mouse_event(src, CGEventType::MouseMoved, pos, CGMouseButton::Left)
    //     .map_err(|_| "Failed to create CGEvent")?;

    // //post it to the system
    // move_event.post(CGEventTapLocation::HID);

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
    println!("Key down: {:?}", payload);

    Ok(())
}

pub fn handle_key_up(payload: &[u8]) -> Result<(), Box<dyn Error>>  {
    println!("Key up: {:?}", payload);

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