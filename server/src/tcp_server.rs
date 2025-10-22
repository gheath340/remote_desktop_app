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


#[inline]
fn rgba_to_rgb_inplace(dst_rgb: &mut [u8], src_rgba: &[u8]) {
    // dst_rgb must be (width * height * 3) bytes long
    let mut di = 0;
    for chunk in src_rgba.chunks_exact(4) {
        dst_rgb[di..di + 3].copy_from_slice(&chunk[0..3]);
        di += 3;
    }
}

fn downscale_rgba_box_2x(dst: &mut [u8], src: &[u8], w: usize, h: usize) -> (usize, usize) {
    let nw = w / 2;
    let nh = h / 2;
    for y in 0..nh {
        for x in 0..nw {
            let mut r:u32=0; let mut g:u32=0; let mut b:u32=0; let mut a:u32=0;
            for dy in 0..2 {
                for dx in 0..2 {
                    let i = ((2*y+dy)*w + (2*x+dx))*4;
                    r += src[i+0] as u32;
                    g += src[i+1] as u32;
                    b += src[i+2] as u32;
                    a += src[i+3] as u32;
                }
            }
            let o = (y*nw + x)*4;
            dst[o+0]=(r/4) as u8; dst[o+1]=(g/4) as u8; dst[o+2]=(b/4) as u8; dst[o+3]=(a/4) as u8;
        }
    }
    (nw, nh)
}

//TO RUN YDOTOOLD(to allow for mouse and keyboard input) run "~/bin/ydotool_session.sh" in empty terminal window
//run "sudo pkill -f ydotoold" to stop ydotoold
fn handle_client(mut tcp: TcpStream, tls_config: Arc<ServerConfig>) -> Result<(), Box<dyn std::error::Error>> {
    tcp.set_nodelay(true)?;
    tcp.set_read_timeout(Some(Duration::from_millis(5)))?;
    tcp.set_write_timeout(Some(Duration::from_millis(5)))?;

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

    //start ScreenCaptureKit capture
    let rx = start_sck_stream();
    println!("ScreenCaptureKit capture started…");

    //get first image and the images width/height
    let (init_width, init_height, first_rgba) = rx.recv()?;
    //create an empty vec with dimensions of first image as RGB
    let mut rgb_buf = vec![0u8; (init_width / 2) * (init_height / 2) * 3];
    //create an empty vec with dimensions of first image downscaled
    let mut down_rgba = vec![0u8; (init_width / 2) * (init_height / 2) * 4];

    let (width, height) = downscale_rgba_box_2x(&mut down_rgba, &first_rgba, init_width, init_height);
    // convert the downscaled RGBA to RGB
    rgba_to_rgb_inplace(&mut rgb_buf[0..width*height*3], &down_rgba[0..width*height*4]);

    //create rgb of first image
    //rgba_to_rgb_inplace(&mut rgb_buf, &first_rgba);

    //set current encoder dimensions
    let mut current_enc_w = width;
    let mut current_enc_h = height;

    //set h.264 encoder configuration then build encoder with that config
    let enc_cfg = EncoderConfig::new(width as u32, height as u32)
        .max_frame_rate(30.0)
        .set_bitrate_bps(10_000_000)
        .rate_control_mode(RateControlMode::Bitrate);
    let mut encoder = Encoder::with_config(enc_cfg)?;

    //converts rgb image into yuv420 format
    let yuv = YUVBuffer::with_rgb(width as usize, height as usize, &rgb_buf);

    //compresses yuv image into bitstream
    let bitstream = encoder.encode(&yuv)?;
    //clones bitstream into vec<u8> so it can be sent over TLS
    let encoded_bytes = bitstream.to_vec();

    //sends info to frame_receiver
    frame_transmitter.send((MessageType::FrameDelta, encoded_bytes))?;
    frame_transmitter.send((MessageType::FrameEnd, Vec::new()))?;

    //initialize prev_frame with the first frame
    let mut prev_frame = first_rgba;

    loop {
        let mut latest = None;
        // drain the capture channel and keep only the latest frame
        while let Ok(frame) = rx.try_recv() {
            latest = Some(frame);
        }

        if let Some((_, _, rgba)) = latest {
            // Downscale
            let (nw, nh) = downscale_rgba_box_2x(&mut down_rgba, &rgba, init_width, init_height);
            // Convert RGBA → RGB
            rgba_to_rgb_inplace(
                &mut rgb_buf[0..nw * nh * 3],
                &down_rgba[0..nw * nh * 4],
            );

            // Prepare YUV buffer and encode
            let yuv = YUVBuffer::with_rgb(nw, nh, &rgb_buf[0..nw * nh * 3]);
            let bitstream = encoder.encode(&yuv)?;
            let encoded = bitstream.to_vec();
            if !encoded.is_empty() {
                frame_transmitter.send((MessageType::FrameDelta, encoded))?;
                frame_transmitter.send((MessageType::FrameEnd, Vec::new()))?;
            }
        }
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
        //if frame was tramsitted from main loop, send it to client
        while let Ok((msg_type, payload)) = frame_receiver.try_recv() {
            send_response(tls, msg_type, &payload)?;
            sent_any = true;
        }

        // If we sent frames, go right back to loop to drain quickly
        if sent_any {
            continue;
        }

        //Try to read, but don't block forever
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
                // read payload and send to match to get handled properly
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
    }
}

pub fn send_response<T: Write>(stream: &mut T, msg_type: MessageType, payload: &[u8],) -> Result<(), Box<dyn std::error::Error>> {
    //build a single buffer (header + payload)
    let mut buf = Vec::with_capacity(5 + payload.len());
    buf.push(msg_type.to_u8());
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(payload);

    //closure to handle WouldBlock
    //keeps retrying until all bytes are written
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
        Ok(())
    };

    //send the message to client
    write_all_retry(&buf)?;
    let _ = stream.flush();
    Ok(())
}