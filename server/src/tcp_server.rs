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

//TO RUN YDOTOOLD(to allow for mouse and keyboard input) run "~/bin/ydotool_session.sh" in empty terminal window
//run "sudo pkill -f ydotoold" to stop ydotoold
fn handle_client(mut tcp: TcpStream, tls_config: Arc<ServerConfig>) -> Result<(), Box<dyn std::error::Error>> {
    tcp.set_nodelay(true)?;
    tcp.set_nonblocking(true)?;
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

    // Send that as the initial full frame
    {
        let mut compressor = Compressor::new()?;
        compressor.set_subsamp(Subsamp::Sub2x2)?;
        compressor.set_quality(80)?;
        let mut output = OutputBuf::new_owned();

        let image = Image {
            pixels: first_rgba.as_slice(),
            width,
            pitch: width * 4,
            height,
            format: PixelFormat::RGBA,
        };
        compressor.compress(image, &mut output)?;
        let jpeg = output.as_ref().to_vec();

        // Kickstart the client: Full frame + FrameEnd (FrameEnd also forces a flush on your send_response)
        frame_transmitter.send((MessageType::FrameFull, jpeg))?;
        frame_transmitter.send((MessageType::FrameEnd, Vec::new()))?;
    }

    // Initialize prev_frame with the first frame so deltas work
    let mut prev_frame = first_rgba;

    loop {
        let loop_timer = Instant::now();
        let mut latest = None;

        // drain the capture channel and keep only the latest frame
        while let Ok(frame) = rx.try_recv() {
            latest = Some(frame);
        }

        if let Some((_, _, rgba)) = latest {
            // offline delta generation — this version just returns Vec<u8>
            match message_type_handlers::handle_frame_delta(
                &mut prev_frame,
                width,
                height,
                rgba,
            ) {
                Ok((msg_type, payload)) => {
                    frame_transmitter.send((msg_type, payload))?;
                    frame_transmitter.send((MessageType::FrameEnd, Vec::new()))?;

                }
                Err(e) => eprintln!("Frame processing error: {e}"),
            }

            println!("Frame loop time: {}ms", loop_timer.elapsed().as_millis());
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

//takes info from client and dispatches to correct MessageType handler
fn dispatcher<T: Read + Write>(tls: &mut T, frame_receiver: mpsc::Receiver<(MessageType, Vec<u8>)>) -> Result<(), Box<dyn Error>> {
        loop {
            let timer = Instant::now();
            // --- Send any pending frames ---
            while let Ok((msg_type, payload)) = frame_receiver.try_recv() {
                send_response(tls, msg_type, &payload)?;
                println!("Dispatcher timer: {}ms", timer.elapsed().as_millis());
            }

            // --- Try to read incoming messages ---
            let mut header = [0u8; 5];
            match tls.read_exact(&mut header) {
                Ok(_) => {
                    let msg_type = MessageType::from_u8(header[0]);
                    let payload_len =
                        u32::from_be_bytes([header[1], header[2], header[3], header[4]]);
                    let mut payload = vec![0u8; payload_len as usize];
                    tls.read_exact(&mut payload)?;

                    match msg_type {
                        MessageType::Text => message_type_handlers::handle_text(&payload)?,
                        MessageType::Connect => message_type_handlers::handle_connect(&payload)?,
                        MessageType::Disconnect => message_type_handlers::handle_disconnect(&payload)?,
                        MessageType::Error => message_type_handlers::handle_error(&payload)?,

                        //MessageType::FrameFull => message_type_handlers::handle_frame_full(tls)?,
                        //MessageType::FrameDelta => message_type_handlers::handle_frame_delta(tls)?,
                        MessageType::FrameFull => {}
                        MessageType::FrameDelta => {}
                        MessageType::FrameEnd => {}
                        MessageType::CursorShape => message_type_handlers::handle_cursor_shape(&payload)?,
                        MessageType::CursorPos => message_type_handlers::handle_cursor_pos(&payload)?,
                        MessageType::Resize => message_type_handlers::handle_resize(&payload)?,

                        MessageType::KeyDown => message_type_handlers::handle_key_down(&payload)?,
                        MessageType::KeyUp => message_type_handlers::handle_key_up(&payload)?,
                        MessageType::MouseMove => message_type_handlers::handle_mouse_move(&payload)?,
                        MessageType::MouseDown => message_type_handlers::handle_mouse_down(&payload)?,
                        MessageType::MouseUp => message_type_handlers::handle_mouse_up(&payload)?,
                        MessageType::MouseScroll => message_type_handlers::handle_mouse_scroll(&payload)?,

                        MessageType::Clipboard => message_type_handlers::handle_clipboard(&payload)?,


                        MessageType::Unknown(code) => {
                            println!("Unknown message type: {code:#X}, skipping {payload_len} bytes");
                        }
                    }
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(ref e) if e.kind() == ErrorKind::UnexpectedEof => {
                    println!("Client disconnected");
                    break;
                }
                Err(e) => return Err(Box::new(e)),
            }

            // Avoid pegging a CPU core
            std::thread::sleep(Duration::from_millis(1));
        }

    Ok(())
}

pub fn send_response<T: Write>( stream: &mut T, msg_type: MessageType, payload: &[u8],) -> Result<(), Box<dyn Error>> {
    let type_byte = msg_type.to_u8();
    let len_bytes = (payload.len() as u32).to_be_bytes();

    // Helper closure to handle writes that may return WouldBlock
    let mut write_all_retry = |data: &[u8]| -> Result<(), Box<dyn Error>> {
        let mut offset = 0;
        while offset < data.len() {
            match stream.write(&data[offset..]) {
                Ok(0) => return Err("Socket closed while writing".into()),
                Ok(n) => offset += n,
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    // Socket not ready yet — back off and retry
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => return Err(Box::new(e)),
            }
        }
        Ok(())
    };

    // write the header + payload with retry
    write_all_retry(&[type_byte])?;
    write_all_retry(&len_bytes)?;
    write_all_retry(payload)?;

    if msg_type == MessageType::FrameEnd {
        // flush may also hit WouldBlock, so handle it the same way
        loop {
            match stream.flush() {
                Ok(_) => break,
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => return Err(Box::new(e)),
            }
        }
    }
    Ok(())
}

