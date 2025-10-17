use std::{
    io::{ Read, Write, ErrorKind, },
    error::Error,
    sync::{ Arc },
    env,
    net::{ TcpListener, TcpStream, },
    time::{ Instant, Duration },
    thread,
    sync::{ mpsc, },
};
use rustls::{
    ServerConfig,
    ServerConnection,
    Stream,
    StreamOwned
};
use turbojpeg::{ Compressor,
    Image,
    PixelFormat,
    Subsamp,
    OutputBuf,
};
use common::message_type::MessageType;
use crate::message_type_handlers;
use crate::capture::start_sck_stream;
use openh264::{ encoder::{Encoder, EncoderConfig}, formats::YUVBuffer };

// fn rgba_to_i420(rgba: &[u8], out: &mut [u8], width: usize, height: usize) {
//     let frame_size = width * height;
//     let (y_plane, uv_planes) = out.split_at_mut(frame_size);
//     let (u_plane, v_plane) = uv_planes.split_at_mut(frame_size / 4);

//     for j in 0..height {
//         for i in 0..width {
//             let idx = (j * width + i) * 4;
//             let r = rgba[idx] as f32;
//             let g = rgba[idx + 1] as f32;
//             let b = rgba[idx + 2] as f32;

//             // BT.601-ish
//             let y = 0.257 * r + 0.504 * g + 0.098 * b + 16.0;
//             y_plane[j * width + i] = y as u8;

//             if (j & 1) == 0 && (i & 1) == 0 {
//                 let u = -0.148 * r - 0.291 * g + 0.439 * b + 128.0;
//                 let v =  0.439 * r - 0.368 * g - 0.071 * b + 128.0;
//                 let uv_idx = (j / 2) * (width / 2) + (i / 2);
//                 u_plane[uv_idx] = u as u8;
//                 v_plane[uv_idx] = v as u8;
//             }
//         }
//     }
// }
fn rgba_to_rgb(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 4;
            rgb.push(rgba[i]);
            rgb.push(rgba[i + 1]);
            rgb.push(rgba[i + 2]);
        }
    }
    rgb
}

fn dump_nal_types_annexb(buf: &[u8]) {
    // Walk start-code delimited NALs and print their types
    let mut i = 0;
    while i + 4 <= buf.len() {
        // find start code
        if i + 4 <= buf.len() && &buf[i..i+4] == [0,0,0,1] {
            i += 4;
            if i >= buf.len() { break; }
            let nal = buf[i];
            let nal_type = nal & 0x1F;
            println!("  NAL type: {}", nal_type);
            // advance to next start code
            // naive scan forward until next 0 0 0 1
            let mut j = i + 1;
            while j + 4 <= buf.len() && &buf[j..j+4] != [0,0,0,1] { j += 1; }
            i = j;
        } else {
            i += 1;
        }
    }
}


//TO RUN YDOTOOLD(to allow for mouse and keyboard input) run "~/bin/ydotool_session.sh" in empty terminal window
//run "sudo pkill -f ydotoold" to stop ydotoold
fn handle_client(mut tcp: TcpStream, tls_config: Arc<ServerConfig>) -> Result<(), Box<dyn std::error::Error>> {
    tcp.set_nodelay(true)?;
    tcp.set_read_timeout(Some(Duration::from_millis(5)))?;
    tcp.set_write_timeout(Some(Duration::from_millis(5)))?;
    //tcp.set_nonblocking(true)?;
    // --- Create TLS stream ---
    let mut tls_conn = ServerConnection::new(tls_config.clone())?;
    loop {
        match tls_conn.complete_io(&mut tcp) {
            Ok((_rd, _wr)) => {
                // Handshake complete when both conditions true:
                if !tls_conn.is_handshaking() {
                    break;
                }
            }

            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Socket not ready yet — wait a bit and try again
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }

            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {
                // Interrupted by signal, just retry
                continue;
            }

            Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                // Client disconnected before handshake finished
                return Err("Client disconnected during TLS handshake".into());
            }

            Err(e) => {
                // Any other error is fatal
                return Err(Box::new(e));
            }
        }

        // Optional: back off a bit if handshake is still progressing
        std::thread::sleep(Duration::from_millis(1));
    }
    let tls = StreamOwned::new(tls_conn, tcp);

    println!("New client connection");

    let (frame_transmitter, frame_receiver) = std::sync::mpsc::channel::<(MessageType, Vec<u8>)>();

    //new dispatcher thread
    std::thread::spawn(move || {
    // this thread owns the TLS stream
    let mut tls = tls;
    if let Err(e) = dispatcher(&mut tls, frame_receiver) {
        eprintln!("Dispatcher thread error: {e}");
    }
    });

    // --- Start ScreenCaptureKit capture ---
    let rx = start_sck_stream();
    println!("ScreenCaptureKit capture started…");

    let (width, height, first_rgba) = rx.recv()?;
    let first_rgb = rgba_to_rgb(&first_rgba, width, height);
    assert_eq!(first_rgb.len(), width as usize * height as usize * 3, "First RGB size mismatch");

    // Build encoder
    let mut encoder = Encoder::with_config(
        EncoderConfig::new(width as u32, height as u32)
            .max_frame_rate(30.0)
            .set_bitrate_bps(5_000_000)
            .debug(false)
    )?;

    let mut frame_count: u32 = 0;
    // Convert directly to YUV (the library handles RGB→YUV internally)
    let yuv = YUVBuffer::with_rgb(width as usize, height as usize, &first_rgb);

    // Encode the frame
    let bitstream = encoder.encode(&yuv)?;
    let encoded_bytes = bitstream.to_vec();
    dump_nal_types_annexb(&encoded_bytes);
    println!("Encoded frame size: {} bytes", encoded_bytes.len());


    // Send it to client
    frame_transmitter.send((MessageType::FrameDelta, encoded_bytes))?;
    frame_transmitter.send((MessageType::FrameEnd, Vec::new()))?;

    // Send that as the initial full frame
    // {
    //     let mut compressor = Compressor::new()?;
    //     compressor.set_subsamp(Subsamp::Sub2x2)?;
    //     compressor.set_quality(80)?;
    //     let mut output = OutputBuf::new_owned();

    //     let image = Image {
    //         pixels: first_rgba.as_slice(),
    //         width,
    //         pitch: width * 4,
    //         height,
    //         format: PixelFormat::RGBA,
    //     };
    //     compressor.compress(image, &mut output)?;
    //     let jpeg = output.as_ref().to_vec();

    //     // Kickstart the client: Full frame + FrameEnd (FrameEnd also forces a flush on your send_response)
    //     frame_transmitter.send((MessageType::FrameFull, jpeg))?;
    //     frame_transmitter.send((MessageType::FrameEnd, Vec::new()))?;
    // }



    // Initialize prev_frame with the first frame so deltas work
    let mut prev_frame = first_rgba;

    loop {
        let loop_timer = Instant::now();
        let mut latest = None;

        // drain the capture channel and keep only the latest frame
        let timer1 = Instant::now();
        while let Ok(frame) = rx.try_recv() {
            latest = Some(frame);
            println!("Frame loop rx.try_recv time: {}ms", timer1.elapsed().as_millis());

        }

        if let Some((_, _, rgba)) = latest {
            if rgba.len() != width * height * 4 {
                eprintln!( "⚠️ Frame size mismatch: rgba.len() = {}, expected = {} ({}x{})",rgba.len(),width * height * 4,width,height);
            } else {
                println!("✅ Frame size OK: {} bytes ({}x{})", rgba.len(), width, height);
            }
            let t_encode = Instant::now();
            let rgb = rgba_to_rgb(&rgba, width, height);
            assert_eq!(rgb.len(), width as usize * height as usize * 3, "RGB size mismatch");

            let yuv = YUVBuffer::with_rgb(width as usize, height as usize, &rgb);
            if frame_count % 60 == 0 {
                encoder = Encoder::with_config(
                EncoderConfig::new(width as u32, height as u32)
                    .max_frame_rate(30.0)
                    .set_bitrate_bps(5_000_000)
            )?;
            }

            // Encode the frame
            let bitstream = encoder.encode(&yuv)?;
            let mut encoded = bitstream.to_vec();
            dump_nal_types_annexb(&encoded);
            println!("Encoded frame size: {} bytes", encoded.len());

            if !encoded.is_empty() {
                frame_transmitter.send((MessageType::FrameDelta, encoded))?;
                frame_transmitter.send((MessageType::FrameEnd, Vec::new()))?;
            }

            println!("encode+send: {}ms", t_encode.elapsed().as_millis());
        }
        // if let Some((_, _, rgba)) = latest {
        //     // offline delta generation — this version just returns Vec<u8>
        //     let timer2 = Instant::now();
        //     match message_type_handlers::handle_frame_delta(
        //         &mut prev_frame,
        //         width,
        //         height,
        //         rgba,
        //     ) {
        //         Ok((msg_type, payload)) => {
        //             frame_transmitter.send((msg_type, payload))?;
        //             frame_transmitter.send((MessageType::FrameEnd, Vec::new()))?;
        //             println!("Frame loop handle_frame_delta time: {}ms", timer2.elapsed().as_millis());
        //         }
        //         Err(e) => eprintln!("Frame processing error: {e}"),
        //     }

        //     println!("Frame full loop time: {}ms", loop_timer.elapsed().as_millis());
        // }

        thread::sleep(std::time::Duration::from_millis(16));
    }
}

//to run on local host SERVER_BIND=127.0.0.1:7878 cargo run --release -p server
//to run at on vm at work or at home cargo run --release -p server
pub fn run(tls_config: Arc<ServerConfig>) -> Result<(), Box<dyn Error>> {
    let default_addr = "0.0.0.0:7878".to_string();
    //allow override of bind address with env var
    let bind_addr = env::var("SERVER_BIND").unwrap_or(default_addr);
    let listener = TcpListener::bind(&bind_addr)?;
    println!("Tcp server listening to {bind_addr}");
    //call handel_client on all clients that contact tcp adress
    for stream in listener.incoming() {
        handle_client(stream?, tls_config.clone())?;
    }
    Ok(())
}

fn handle_incoming_message(msg_type: MessageType, payload: &[u8]) -> Result<(), Box<dyn Error>> {
    match msg_type {
        MessageType::Text => message_type_handlers::handle_text(payload)?,
        MessageType::Connect => message_type_handlers::handle_connect(payload)?,
        MessageType::Disconnect => message_type_handlers::handle_disconnect(payload)?,
        MessageType::Error => message_type_handlers::handle_error(payload)?,

        MessageType::CursorShape => message_type_handlers::handle_cursor_shape(payload)?,
        MessageType::CursorPos => message_type_handlers::handle_cursor_pos(payload)?,
        MessageType::Resize => message_type_handlers::handle_resize(payload)?,

        MessageType::KeyDown => message_type_handlers::handle_key_down(payload)?,
        MessageType::KeyUp => message_type_handlers::handle_key_up(payload)?,
        MessageType::MouseMove => message_type_handlers::handle_mouse_move(payload)?,
        MessageType::MouseDown => message_type_handlers::handle_mouse_down(payload)?,
        MessageType::MouseUp => message_type_handlers::handle_mouse_up(payload)?,
        MessageType::MouseScroll => message_type_handlers::handle_mouse_scroll(payload)?,

        MessageType::Clipboard => message_type_handlers::handle_clipboard(payload)?,

        MessageType::FrameFull => {}
        MessageType::FrameDelta => {}
        MessageType::FrameEnd => {}

        MessageType::Unknown(code) => {
            println!("Unknown message type: {code:#X}, skipping {} bytes", payload.len());
        }
    }

    Ok(())
}

fn dispatcher<T: Read + Write>(tls: &mut T, frame_receiver: mpsc::Receiver<(MessageType, Vec<u8>)>) -> Result<(), Box<dyn Error>> {

    let mut header = [0u8; 5];

    loop {
        // 1. Try to send any frames waiting in the channel
        // while let Ok((msg_type, payload)) = frame_receiver.try_recv() {
        //     send_response(tls, msg_type, &payload)?;
        // }
        // match frame_receiver.recv_timeout(Duration::from_millis(5)) {
        //     Ok((msg_type, payload)) => {
        //         let now = Instant::now();
        //         println!("[DISPATCH SEND] {:?} | payload {} bytes | {:?}", msg_type, payload.len(), now);
        //         // send the one we waited for...
        //         send_response(tls, msg_type, &payload)?;
        //         // ...then drain any backlog arriving at the same time
        //         while let Ok((mt, pl)) = frame_receiver.try_recv() {
        //             println!("[DISPATCH SEND] {:?} (backlog) | payload {} bytes | {:?}",mt,pl.len(),Instant::now());
        //             send_response(tls, mt, &pl)?;
        //         }
        //     }
        //     Err(mpsc::RecvTimeoutError::Timeout) => {
        //         // no outgoing frames right now; that's fine — we'll probe for inbound below
        //     }
        //     Err(mpsc::RecvTimeoutError::Disconnected) => {
        //         // producer died; end dispatcher
        //         return Ok(());
        //     }
        // }
        let mut sent_any = false;
        while let Ok((msg_type, payload)) = frame_receiver.try_recv() {
            println!("[DISPATCH SEND] {:?} | {} bytes | {:?}", msg_type, payload.len(), Instant::now());
            send_response(tls, msg_type, &payload)?;
            sent_any = true;
        }

        // If we sent frames, go right back to loop to drain more quickly
        if sent_any {
            continue;
        }

        // 2. Try to read, but don't block forever
        match tls.read(&mut header) {
            Ok(0) => {
                println!("Client disconnected");
                return Ok(());
            }
            Ok(n) if n < 5 => {
                // partial read; read the rest next loop iteration
                continue;
            }
            Ok(_) => {
                // read payload normally
                let msg_type = MessageType::from_u8(header[0]);
                let payload_len =
                    u32::from_be_bytes([header[1], header[2], header[3], header[4]]);
                let mut payload = vec![0u8; payload_len as usize];
                tls.read_exact(&mut payload)?;

                handle_incoming_message(msg_type, &payload)?;
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                // no incoming data yet, continue loop
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            Err(ref e) if e.kind() == ErrorKind::TimedOut => {
                continue;
            }
            Err(ref e) if e.kind() == ErrorKind::Interrupted => {
                continue;
            }
            Err(e) => return Err(Box::new(e)),
        }
        std::thread::sleep(Duration::from_micros(500));
    }
}

//takes info from client and dispatches to correct MessageType handler
// fn dispatcher<T: Read + Write>(tls: &mut T, frame_receiver: mpsc::Receiver<(MessageType, Vec<u8>)>) -> Result<(), Box<dyn Error>> {
//         loop {
//             let timer = Instant::now();
//             // --- Send any pending frames ---
//             while let Ok((msg_type, payload)) = frame_receiver.try_recv() {
//                 send_response(tls, msg_type, &payload)?;
//                 println!("Dispatcher timer: {}ms", timer.elapsed().as_millis());
//             }

//             // --- Try to read incoming messages ---
//             let mut header = [0u8; 5];
//             match tls.read_exact(&mut header) {
//                 Ok(_) => {
//                     let msg_type = MessageType::from_u8(header[0]);
//                     let payload_len =
//                         u32::from_be_bytes([header[1], header[2], header[3], header[4]]);
//                     let mut payload = vec![0u8; payload_len as usize];
//                     tls.read_exact(&mut payload)?;

//                     match msg_type {
//                         MessageType::Text => message_type_handlers::handle_text(&payload)?,
//                         MessageType::Connect => message_type_handlers::handle_connect(&payload)?,
//                         MessageType::Disconnect => message_type_handlers::handle_disconnect(&payload)?,
//                         MessageType::Error => message_type_handlers::handle_error(&payload)?,

//                         //MessageType::FrameFull => message_type_handlers::handle_frame_full(tls)?,
//                         //MessageType::FrameDelta => message_type_handlers::handle_frame_delta(tls)?,
//                         MessageType::FrameFull => {}
//                         MessageType::FrameDelta => {}
//                         MessageType::FrameEnd => {}
//                         MessageType::CursorShape => message_type_handlers::handle_cursor_shape(&payload)?,
//                         MessageType::CursorPos => message_type_handlers::handle_cursor_pos(&payload)?,
//                         MessageType::Resize => message_type_handlers::handle_resize(&payload)?,

//                         MessageType::KeyDown => message_type_handlers::handle_key_down(&payload)?,
//                         MessageType::KeyUp => message_type_handlers::handle_key_up(&payload)?,
//                         MessageType::MouseMove => message_type_handlers::handle_mouse_move(&payload)?,
//                         MessageType::MouseDown => message_type_handlers::handle_mouse_down(&payload)?,
//                         MessageType::MouseUp => message_type_handlers::handle_mouse_up(&payload)?,
//                         MessageType::MouseScroll => message_type_handlers::handle_mouse_scroll(&payload)?,

//                         MessageType::Clipboard => message_type_handlers::handle_clipboard(&payload)?,


//                         MessageType::Unknown(code) => {
//                             println!("Unknown message type: {code:#X}, skipping {payload_len} bytes");
//                         }
//                     }
//                 }
//                 Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
//                     std::thread::sleep(Duration::from_millis(2));
//                     continue;
//                 }
//                 Err(ref e) if e.kind() == ErrorKind::UnexpectedEof => {
//                     println!("Client disconnected");
//                     break;
//                 }
//                 Err(e) => return Err(Box::new(e)),
//             }

//             // Avoid pegging a CPU core
//             std::thread::sleep(Duration::from_millis(1));
//         }

//     Ok(())
// }

pub fn send_response<T: Write>(stream: &mut T, msg_type: MessageType, payload: &[u8],) -> Result<(), Box<dyn std::error::Error>> {
    let timer = Instant::now();

    // Build a single contiguous buffer (header + payload)
    let mut buf = Vec::with_capacity(5 + payload.len());
    buf.push(msg_type.to_u8());
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(payload);

    // Helper closure to handle WouldBlock with retries
    let mut write_all_retry = |data: &[u8]| -> Result<(), Box<dyn std::error::Error>> {
        let mut offset = 0;
        let mut retries = 0;
        while offset < data.len() {
            match stream.write(&data[offset..]) {
                Ok(0) => return Err("Socket closed while writing".into()),
                Ok(n) => offset += n,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    retries += 1;
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => return Err(Box::new(e)),
            }
        }
        if retries > 0 {
            println!("WouldBlock retried {retries} times while writing {} bytes", data.len());
        }
        Ok(())
    };

    // Single TLS record write — no multi-part small writes
    write_all_retry(&buf)?;
    let _ = stream.flush();

    // Optional flush for frame end — though usually unnecessary for rustls
    // if msg_type == MessageType::FrameEnd {
    //     let mut retries = 0;
    //     loop {
    //         match stream.flush() {
    //             Ok(_) => break,
    //             Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
    //                 retries += 1;
    //                 std::thread::sleep(Duration::from_millis(1));
    //                 continue;
    //             }
    //             Err(e) => return Err(Box::new(e)),
    //         }
    //     }
    //     if retries > 0 {
    //         println!("Flush retried {retries} times");
    //     }
    // }

    println!("send_response done in {}ms", timer.elapsed().as_millis());
    Ok(())
}



// pub fn send_response<T: Write>( stream: &mut T, msg_type: MessageType, payload: &[u8],) -> Result<(), Box<dyn Error>> {
//     let timer = Instant::now();
//     let type_byte = msg_type.to_u8();
//     let len_bytes = (payload.len() as u32).to_be_bytes();

//     // Helper closure to handle writes that may return WouldBlock
//     let mut write_all_retry = |data: &[u8]| -> Result<(), Box<dyn Error>> {
//         let timer1 = Instant::now();
//         let mut offset = 0;
//         while offset < data.len() {
//             match stream.write(&data[offset..]) {
//                 Ok(0) => return Err("Socket closed while writing".into()),
//                 Ok(n) => offset += n,
//                 Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
//                     // Socket not ready yet — back off and retry
//                     std::thread::sleep(Duration::from_millis(1));
//                     continue;
//                 }
//                 Err(e) => return Err(Box::new(e)),
//             }
//             println!("write_all_retry timer: {}ms", timer.elapsed().as_millis());
//         }
//         Ok(())
//     };

//     // write the header + payload with retry
//     let timer1 = Instant::now();
//     write_all_retry(&[type_byte])?;
//     write_all_retry(&len_bytes)?;
//     write_all_retry(payload)?;
//     println!("Writing all in send response timer: {}ms", timer1.elapsed().as_millis());


//     if msg_type == MessageType::FrameEnd {
//         // flush may also hit WouldBlock, so handle it the same way
//         let timer2 = Instant::now();
//         loop {
//             match stream.flush() {
//                 Ok(_) => break,
//                 Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
//                     std::thread::sleep(Duration::from_millis(1));
//                     continue;
//                 }
//                 Err(e) => return Err(Box::new(e)),
//             }
//             println!("Frame end timer: {}ms", timer2.elapsed().as_millis());

//         }
//     }
//     Ok(())
// }