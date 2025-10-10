use common::message_type::MessageType;
use std::{ 
    process,
    net::TcpStream,
    io::{ Write, Read, },
    error::Error,
    sync::{ Arc, mpsc },
    time::{ Instant, Duration },
    env,
};
use rustls::{ 
    ClientConfig, 
    ClientConnection, 
    Stream, 
    pki_types::ServerName,
 };
use winit::{ 
    event_loop::{ EventLoopBuilder, ControlFlow, EventLoopProxy },
    event::{ Event, WindowEvent },
    window::WindowBuilder,
 };
use pixels::{ SurfaceTexture, Pixels };
use crate::{ message_type_handlers, };
use lz4_flex::decompress_size_prepended;


 #[derive(Debug)]
pub enum UserEvent {
    NewUpdate,
    Redraw,
}

pub enum FrameUpdate {
    Full{ w: u32, h: u32, bytes: Vec<u8> },
    Delta(Vec<u8>),
}

//to run on local host SERVER_ADDR=127.0.0.1:7878 cargo run --release -p client
//to run on vm at home comment out work_address and change connection_address to work_address.clone()
//to run on vmat work comment out home_address and change connection_address to work_address.clone()
pub fn run(tls_config: Arc<ClientConfig>) -> Result<(), Box<dyn Error>> {
    //let home_address = "192.168.50.209:7878".to_string();
    let work_address = "10.176.7.73:7878".to_string();
    //allow for server address override by calling "SERVER_ADDR=<address> cargo run -p client"
    let connection_address = env::var("SERVER_ADDR").unwrap_or(work_address.clone());
    println!("Connecting to server at {}", connection_address);


    //create the UI, main thread loop
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    //the proxy that allows dispatcher thread to send message to event_loop
    let proxy = event_loop.create_proxy();

    //create the transmitter and reciever for the mpsc channel(message queue) that carries messages of the type FrameUpdate
    let (channel_transmitter, channel_reciever) = mpsc::channel::<FrameUpdate>();

    //let addr_str = connection_address.clone();

    //create thread for dispatcher
    std::thread::spawn(move || {
        //create tcp connection
        let addr_str = connection_address.clone();
        let mut tcp = TcpStream::connect(&addr_str)
            .unwrap_or_else(|e| {
                eprintln!("Failed to create TCP connection: {e}");
                process::exit(1);
            });
            tcp.set_nodelay(true).expect("set_nodelay failed");

        //get hostname of server
        let server_name_str = addr_str.split(':').next().unwrap_or("localhost").to_string();
        let server_name = ServerName::try_from(server_name_str)
            .unwrap_or_else(|e| {
                eprintln!("Invalid server name: {e}");
                process::exit(1);
            });
        //create TLS client state machine
        let mut tls_connection = ClientConnection::new(tls_config, server_name)
            .unwrap_or_else(|e| {
                eprintln!("Failed to create TLS connection: {e}");
                process::exit(1);
            });
        //create a TLS stream
        let mut tls = Stream::new(&mut tls_connection, &mut tcp);

        if let Err(e) = dispatcher(&mut tls, channel_transmitter, proxy) {
            eprintln!("Dispatcher error: {e}");
        }
    });

    //looping until the channel recieves a full frame update from dispatcher thread
    let (width, height, first_rgba) = loop {
        match channel_reciever.recv()? {
            FrameUpdate::Full{ w, h, bytes } => break (w, h, bytes),
            FrameUpdate::Delta(_) => {
                continue;
            }
        }
    };

    //build window
    let window = WindowBuilder::new()
        .with_title("Remote desktop client")
        .build(&event_loop)?;

    //create surface and attatch pixels to it
    let surface_texture = SurfaceTexture::new(width, height, &window);
    let mut pixels = Pixels::new(width, height, surface_texture)?;

    //put image into pixels to display on window
    {
        message_type_handlers::handle_frame_full(width, height, &first_rgba, &mut pixels)?;
        pixels.render().unwrap();
    }

    //used for FPS calculation
    let mut last_frame = Instant::now();
    let mut frame_count = 0u32;

    //run eventloop to correctly handle everything
    event_loop.run(move |event, _, control_flow| {
        //*control_flow = ControlFlow::Wait;
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16));

     let pixel_render_timer = Instant::now();

        //handle all UserEvent types
        match event {
            //handle UserEven::NewUpdates
            Event::UserEvent(UserEvent::NewUpdate) => {
                //check reciever for updates and send to the correct FrameUpdate
                let mut got_any = false;
                while let Ok(update) = channel_reciever.try_recv() {
                    match update {
                        //call handle_frame_full when FrameUpdate::Full
                        FrameUpdate::Full{w, h, bytes} => {
                            if let Err(e) = message_type_handlers::handle_frame_full(w, h, &bytes, &mut pixels) {
                                eprintln!("Frame full error: {e}");
                            }
                            got_any = true;
                        }
                        //call handle_frame_delta when FrameUpdate::Delta
                        FrameUpdate::Delta(bytes) => {
                            if let Err(e) = message_type_handlers::handle_frame_delta(&bytes, &mut pixels) {
                                eprintln!("Frame delta error: {e}");
                            }
                            got_any = true;
                        }
                    }
                }
                //if got_any UserEvent::NewUpdate call for window redraw to apply changes
                if got_any {
                    window.request_redraw();
                }
            },
            //call window request redraw for consistency
            Event::UserEvent(UserEvent::Redraw) => {
                window.request_redraw();
            }
            //window.request_redraw() calls this to redraw window
            Event::RedrawRequested(_) => {
                if let Err(e) = pixels.render() {
                    eprintln!("Render error: {e}");
                }
                println!("Pixel rendered: {}ms", pixel_render_timer.elapsed().as_millis());
                frame_count += 1;
                if last_frame.elapsed() >= Duration::from_secs(1) {
                    println!("FPS: {}", frame_count);
                    frame_count = 0;
                    last_frame = Instant::now();
                }
            }
            //handle WindowEvent::CloseRequested
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => {}
            },
            _ => {}
        }
    });
}

fn dispatcher<T: Read + Write>(tls: &mut T, channel_transmitter: mpsc::Sender<FrameUpdate>, proxy: EventLoopProxy<UserEvent>) -> Result<(), Box<dyn Error>> {
    //create decompressor outside of loop to not recreate one every time
    let mut decompressor = turbojpeg::Decompressor::new()?;
    loop{
        //create header and read data into header
        let mut header = [0u8; 5];
        tls.read_exact(&mut header)?;

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

            MessageType::FrameFull => {
                //inspect the jpeg header to get size without a full decode
                let header = decompressor.read_header(&payload)?;
                let w = header.width as usize;
                let h = header.height as usize;

                //allocate target buffer to accept rgba
                let mut rgba = vec![0u8; w * h * 4];
                //tells the decoder how to handle the pixels
                let out = turbojpeg::Image {
                    pixels: rgba.as_mut_slice(), //mut slice pointing to rgba buffer
                    width: w, //width of jpeg
                    pitch: w * 4, //how many bytes per row(width * 4 for rgba)
                    height: h, //height of jpeg
                    format: turbojpeg::PixelFormat::RGBA, //the format you want the output to be
                };
                //decompresses right into rgba buffer
                decompressor.decompress(&payload, out)?;

                channel_transmitter.send(FrameUpdate::Full{w: w as u32, h: h as u32, bytes: rgba}).ok();
                let _ = proxy.send_event(UserEvent::NewUpdate);
            },
            MessageType::FrameDelta => {
                //decompress image and send it to the UI event loop to be properly handled
                let decompressed = decompress_size_prepended(&payload)?;
                channel_transmitter.send(FrameUpdate::Delta(decompressed)).ok();
                let _ = proxy.send_event(UserEvent::NewUpdate);
            }
            MessageType::FrameEnd => {
                // This is the signal: all deltas applied, now request redraw
                proxy.send_event(UserEvent::Redraw).ok();
            }

            MessageType::Unknown(code) => {
                println!("Unknown message type: {code:#X}, skipping {payload_len} bytes");
            }
        }
    }
}