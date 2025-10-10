use std::{ 
    io::{ Read, Write, ErrorKind, }, 
    error::Error, 
    sync::{ Arc }, 
    env,
    net::{ TcpListener, TcpStream, },
    time::Instant,
};
use rustls::{ 
    ServerConfig, 
    ServerConnection, 
    Stream, 
};
use turbojpeg::{ Compressor, 
    Image, 
    PixelFormat, 
    Subsamp, 
    OutputBuf,
};
use common::message_type::MessageType;
use crate::message_type_handlers;
//use crate::sck::start_sck_stream;
use crate::capture::start_sck_stream;


fn handle_client(mut tcp: TcpStream, tls_config: Arc<ServerConfig>) -> Result<(), Box<dyn std::error::Error>> {
    tcp.set_nodelay(true).expect("set_nodelay failed");
    // --- Create TLS stream ---
    let mut tls_conn = ServerConnection::new(tls_config.clone())?;
    let mut tls = Stream::new(&mut tls_conn, &mut tcp);

    println!("New connection: {:#?}", tls);

    // --- Start ScreenCaptureKit capture ---
    let rx = start_sck_stream();
    println!("ScreenCaptureKit capture started…");

    // --- Wait for first frame ---
    let (width, height, rgba) = rx.recv()?; 
    println!("Got first frame from ScreenCaptureKit");

    let mut rgb = Vec::with_capacity(width * height * 3);
    for chunk in rgba.chunks_exact(4) {
        rgb.extend_from_slice(&chunk[..3]);
    }
    let image = Image {
        pixels: rgb.as_slice(),
        width,
        pitch: width * 3, // bytes per row
        height,
        format: PixelFormat::RGB,
    };
    // Create compressor + output buffer
    let mut compressor = Compressor::new()?;
    let _ = compressor.set_subsamp(Subsamp::Sub2x2);
    let _ = compressor.set_quality(80);
    let mut output = OutputBuf::new_owned();
    // Compress
    compressor.compress(image, &mut output)?;
    let jpeg_data = output.as_ref().to_vec();
    send_response(&mut tls, MessageType::FrameFull, &jpeg_data)?;

    let mut prev_frame = rgba;

    let loop_timer = Instant::now();
    loop {
        let mut latest = None;
        while let Ok(frame) = rx.try_recv() {
            latest = Some(frame);
            println!("Loop: {}ms", loop_timer.elapsed().as_millis());
        }

        if let Some((_, _, rgba)) = latest {
            if let Err(e) = message_type_handlers::handle_frame_delta(&mut tls, &mut prev_frame, width, height, rgba) {
                eprintln!("Stream error: {e}");
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    dispatcher(&mut tls)?;

    Ok(())
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
fn dispatcher<T: Read + Write>(tls: &mut T) -> Result<(), Box<dyn Error>> {
    loop{
        //create header and read data into header
        let mut header = [0u8; 5];
        if let Err(e) = tls.read_exact(&mut header) {
            if e.kind() == ErrorKind::UnexpectedEof {
                println!("Client disconnected");
                break;
            } else {
                return Err(Box::new(e));
            }
        }

        //parse message type and payload_len from header
        let msg_type = MessageType::from_u8(header[0]);
        let payload_len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]);

        //create empty vec that is the appropriate length for payload and fill it wih payload
        let mut payload = vec![0u8; payload_len as usize];
        tls.read_exact(&mut payload)?;

        //dispatch payload to correct handler
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
    Ok(())
}

//sends given message to server
pub fn send_response<T: Write>(stream: &mut T, msg_type: MessageType, payload: &[u8]) -> Result<(), Box<dyn Error>> {
    let send_response_timer = Instant::now();
    //get the byte value from msg_type
    let type_byte = msg_type.to_u8();

    //encode the length of payload into bytes
    let len_bytes = (payload.len() as u32).to_be_bytes();

    //write the header
    stream.write_all(&[type_byte])?;
    stream.write_all(&len_bytes)?;

    //write payload and make sure it goes
    stream.write_all(&payload)?;
    if msg_type == MessageType::FrameEnd {
        stream.flush()?;
    }
    println!("Send response: {}ms", send_response_timer.elapsed().as_millis());

    Ok(())
}
