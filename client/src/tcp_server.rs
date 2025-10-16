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
//use pixels::wgpu::SurfaceSize;
use pixels::wgpu;


 #[derive(Debug)]
pub enum UserEvent {
    NewUpdate,
    Redraw,
}

pub enum FrameUpdate {
    Full{ w: u32, h: u32, bytes: Vec<u8> },
    Delta(Vec<u8>),
}

fn make_mouse_move_packet(x: u32, y: u32) -> Vec<u8> {
    let mut packet = Vec::with_capacity(1 + 4 + 8);

    // 1 byte: message type
    packet.push(MessageType::MouseMove.to_u8());

    // 4 bytes: payload length (always 8 for two u32s)
    packet.extend_from_slice(&8u32.to_be_bytes());

    // 8 bytes: payload (x, y coordinates)
    packet.extend_from_slice(&x.to_be_bytes());
    packet.extend_from_slice(&y.to_be_bytes());

    packet
}

fn calculate_viewport(win_w: u32, win_h: u32, frame_w: u32, frame_h: u32,) -> (u32, u32) {
    let aspect_frame = frame_w as f32 / frame_h as f32;
    let aspect_window = win_w as f32 / win_h as f32;

    if aspect_window > aspect_frame {
        // window is wider than frame, pillarbox horizontally
        let h = win_h;
        let w = (h as f32 * aspect_frame) as u32;
        return (w, h);
    } else {
        // window is taller than frame, letterbox vertically
        let w = win_w;
        let h = (w as f32 / aspect_frame) as u32;
        return (w, h);
    };
}


//to run on local host SERVER_ADDR=127.0.0.1:7878 cargo run --release -p client
//to run on vm at home comment out other _address vars  and change connection_address to vm_work_address.clone()
//to run on vm at work comment out other _address vars  and change connection_address to vm_work_address.clone()
//to run on desktop comment out other _address vars and change connection_address to home_desktop_address.clone()
pub fn run(tls_config: Arc<ClientConfig>) -> Result<(), Box<dyn Error>> {
    let home_desktop_address = "192.168.50.105:7878".to_string();
    //let vm_home_address = "192.168.50.209:7878".to_string();
    //let vm_work_address = "10.176.7.73:7878".to_string();
    //allow for server address override by calling "SERVER_ADDR=<address> cargo run -p client"
    let connection_address = env::var("SERVER_ADDR").unwrap_or(home_desktop_address.clone());
    println!("Connecting to server at {}", connection_address);

    //create the UI, main thread loop
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    //the proxy that allows dispatcher thread to send message to event_loop
    let proxy = event_loop.create_proxy();

    //create the transmitter and reciever for the mpsc channel(message queue) that carries messages of the type FrameUpdate
    let (frame_transmitter, frame_receiver) = mpsc::channel::<FrameUpdate>();
    let (mouse_transmitter, mouse_receiver) = mpsc::channel::<Vec<u8>>();

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

        if let Err(e) = dispatcher(&mut tls, frame_transmitter, proxy, mouse_receiver) {
            eprintln!("Dispatcher error: {e}");
        }
    });

    //looping until the channel recieves a full frame update from dispatcher thread
    let (width, height, first_rgba) = loop {
        match frame_receiver.recv()? {
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
    // let win_size = window.inner_size();
    // let surface_texture = SurfaceTexture::new(win_size.width, win_size.height, &window);
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
                while let Ok(update) = frame_receiver.try_recv() {
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
                // Always resize the pixel surface to match window
                let win_size = window.inner_size();

                // This makes the frame scale to fill the window
                if let Err(e) = pixels.resize_surface(win_size.width, win_size.height) {
                    eprintln!("Resize surface error: {e}");
                }

                // Draw the scaled frame
                if let Err(e) = pixels.render() {
                    eprintln!("Render error: {e}");
                }

                frame_count += 1;
                if last_frame.elapsed() >= Duration::from_secs(1) {
                    println!("FPS: {}", frame_count);
                    frame_count = 0;
                    last_frame = Instant::now();
                }
            }
            Event::WindowEvent { event, .. } => match event {
                //handle window close
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,

                //handle cursor being moved
                WindowEvent::CursorMoved { position, .. } => {
                    //get window size
                    let win_size = window.inner_size();
                    let win_w = win_size.width as f64;
                    let win_h = win_size.height as f64;

                    //get remote screen dimensions
                    let sx = ((position.x / win_w) * width as f64)
                        .round()
                        .clamp(0.0, (width - 1) as f64) as u32;
                    let sy = ((position.y / win_h) * height as f64)
                        .round()
                        .clamp(0.0, (height - 1) as f64) as u32;

                    //build packet and send it
                    let packet = make_mouse_move_packet(sx, sy);
                    let _ = mouse_transmitter.send(packet);
                },
                WindowEvent::Resized(size) => {
                    let (vw, vh) = calculate_viewport(size.width, size.height, width, height);
                    pixels.resize_surface(vw, vh).unwrap();
                    window.request_redraw();
                },
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    let (vw, vh) = calculate_viewport(new_inner_size.width, new_inner_size.height, width, height);
                    pixels.resize_surface(vw, vh).unwrap();
                    window.request_redraw();
                }
                _ => {}
            },
            _ => {}
        }
    });
}

fn dispatcher<T: Read + Write>(tls: &mut T, frame_transmitter: mpsc::Sender<FrameUpdate>, proxy: EventLoopProxy<UserEvent>, mouse_receiver: mpsc::Receiver<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    //create decompressor outside of loop to not recreate one every time
    let mut decompressor = turbojpeg::Decompressor::new()?;
    loop{

        while let Ok(packet) = mouse_receiver.try_recv() {
            println!("Sending mouse packet of length {}", packet.len());
            tls.write_all(&[packet[0]])?;
            tls.write_all(&packet[1..5])?;
            tls.write_all(&packet[5..])?;
            tls.flush()?;
        }
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

                frame_transmitter.send(FrameUpdate::Full{w: w as u32, h: h as u32, bytes: rgba}).ok();
                let _ = proxy.send_event(UserEvent::NewUpdate);
            },
            MessageType::FrameDelta => {
                //decompress image and send it to the UI event loop to be properly handled
                let decompressed = decompress_size_prepended(&payload)?;
                frame_transmitter.send(FrameUpdate::Delta(decompressed)).ok();
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