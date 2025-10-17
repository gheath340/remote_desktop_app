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
use openh264::{ 
    encoder::{ Encoder, EncoderConfig, RateControlMode }, 
    formats::YUVBuffer,
};


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

    // Build encoder
    // let mut encoder = Encoder::with_config(
    //     EncoderConfig::new(width as u32, height as u32)
    //         .max_frame_rate(30.0)
    //         .set_bitrate_bps(5_000_000)
    //         .debug(false)
    // )?;
    let enc_cfg = EncoderConfig::new(width as u32, height as u32)
        .max_frame_rate(30.0)
        .set_bitrate_bps(5_000_000)
        .rate_control_mode(RateControlMode::Bitrate);

    let mut encoder = Encoder::with_config(enc_cfg)?;

    // Convert directly to YUV (the library handles RGB→YUV internally)
    let yuv = YUVBuffer::with_rgb(width as usize, height as usize, &first_rgb);

    // Encode the frame
    let bitstream = encoder.encode(&yuv)?;
    let encoded_bytes = bitstream.to_vec();

    // Send it to client
    frame_transmitter.send((MessageType::FrameDelta, encoded_bytes))?;
    frame_transmitter.send((MessageType::FrameEnd, Vec::new()))?;

    // Initialize prev_frame with the first frame so deltas work
    let mut prev_frame = first_rgba;

    loop {
        let loop_timer = Instant::now();
        let mut latest = None;

        // drain the capture channel and keep only the latest frame
        let timer1 = Instant::now();
        while let Ok(frame) = rx.try_recv() {
            latest = Some(frame);
        }

        if let Some((_, _, rgba)) = latest {
            if rgba.len() != width * height * 4 {
                eprintln!( "⚠️ Frame size mismatch: rgba.len() = {}, expected = {} ({}x{})",rgba.len(),width * height * 4,width,height);
            } else {
                println!("✅ Frame size OK: {} bytes ({}x{})", rgba.len(), width, height);
            }
            let t_encode = Instant::now();
            let rgb = rgba_to_rgb(&rgba, width, height);

            let yuv = YUVBuffer::with_rgb(width as usize, height as usize, &rgb);

            // Encode the frame
            let bitstream = encoder.encode(&yuv)?;
            let mut encoded = bitstream.to_vec();

            if !encoded.is_empty() {
                frame_transmitter.send((MessageType::FrameDelta, encoded))?;
                frame_transmitter.send((MessageType::FrameEnd, Vec::new()))?;
            }
        }

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

        let mut sent_any = false;
        while let Ok((msg_type, payload)) = frame_receiver.try_recv() {
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

    println!("send_response done in {}ms", timer.elapsed().as_millis());
    Ok(())
}