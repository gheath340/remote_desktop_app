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
use pixels::{ SurfaceTexture, Pixels, PixelsBuilder, wgpu };
use crate::{ message_type_handlers, };
use lz4_flex::decompress_size_prepended;
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;


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

fn yuv420p_to_rgba_with_stride(
    y: &[u8], u: &[u8], v: &[u8],
    w: usize, h: usize,
    y_stride: usize, u_stride: usize, v_stride: usize,
) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    let cw = (w + 1) / 2;
    let ch = (h + 1) / 2;

    for j in 0..h {
        let y_row = &y[j * y_stride .. j * y_stride + w];
        let u_row = &u[(j / 2) * u_stride .. (j / 2) * u_stride + cw];
        let v_row = &v[(j / 2) * v_stride .. (j / 2) * v_stride + cw];

        for i in 0..w {
            let yy = y_row[i] as f32;
            let uu = u_row[i / 2] as f32;
            let vv = v_row[i / 2] as f32;

            let c = yy - 16.0;
            let d = uu - 128.0;
            let e = vv - 128.0;

            let r = (1.164 * c + 1.596 * e).clamp(0.0, 255.0) as u8;
            let g = (1.164 * c - 0.392 * d - 0.813 * e).clamp(0.0, 255.0) as u8;
            let b = (1.164 * c + 2.017 * d).clamp(0.0, 255.0) as u8;

            let idx = (j * w + i) * 4;
            out[idx] = r;
            out[idx + 1] = g;
            out[idx + 2] = b;
            out[idx + 3] = 255;
        }
    }
    out
}

//to run on local host SERVER_ADDR=127.0.0.1:7878 cargo run --release -p client
//to run on vm at home comment out other _address vars  and change connection_address to vm_work_address.clone()
//to run on vm at work comment out other _address vars  and change connection_address to vm_work_address.clone()
//to run on desktop comment out other _address vars and change connection_address to home_desktop_address.clone()
pub fn run(tls_config: Arc<ClientConfig>) -> Result<(), Box<dyn Error>> {
    let home_desktop_address = "192.168.50.105:7878".to_string();
    //let vm_home_address = "192.168.50.209:7878".to_string();
    let vm_work_address = "10.176.7.73:7878".to_string();
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
            tcp.set_read_timeout(Some(Duration::from_secs(2))).ok();
            tcp.set_write_timeout(Some(Duration::from_secs(2))).ok();

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

    //loop until the first full image is recieved and grab the image vec and dimensions
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

    //get the size of the initial windows drawable area
    let win_size = window.inner_size();
    //create a gpu-backed surface linked to the window
    let surface_texture = SurfaceTexture::new(win_size.width, win_size.height, &window);
    //creates the pixels renderer that manages the cpu frame buffer(RGBA bytes) and the GPU pipeline that actually uploads and draws to the window
    let mut pixels = Pixels::new(win_size.width, win_size.height, surface_texture)?;

    //handle_frame_full puts the image into the pixels buffer
    //pixels.render draws whats in the pixels buffer onto the screen
    {
        message_type_handlers::handle_frame_full(width, height, &first_rgba, &mut pixels)?;
        pixels.render().unwrap();
    }

    //used for FPS calculation
    let mut last_frame = Instant::now();
    let mut frame_count = 0u32;

    //run eventloop to correctly handle everything
    event_loop.run(move |event, _, control_flow| {
        //tells the event loop to run every 16ms, whether something triggered it or not
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
                    // //get window size
                    // let win_size = window.inner_size();
                    // let win_w = win_size.width as f64;
                    // let win_h = win_size.height as f64;

                    // //get remote screen dimensions
                    // let sx = ((position.x / win_w as f64) * width as f64)
                    //     .round()
                    //     .clamp(0.0, (width - 1) as f64) as u32;
                    // let sy = ((position.y / win_h as f64) * height as f64)
                    //     .round()
                    //     .clamp(0.0, (height - 1) as f64) as u32;

                    // //build packet and send it
                    // let packet = make_mouse_move_packet(sx, sy);
                    // let _ = mouse_transmitter.send(packet);

                    //gets the DPI scale(how many physical pixels each logical pixel is)
                    let scale = window.scale_factor() as f64;
                    //position.x & y are in logical pixels, * by scale to convert to physical pixels
                    let pos_x_px = position.x * scale;
                    let pos_y_px = position.y * scale;

                    //window size in physical pixels
                    let win_size = window.inner_size();
                    let win_w = win_size.width as f64;
                    let win_h = win_size.height as f64;

                    //(pos_x_px / win_w) gives normalized mouse x (0.0 to 1.0) * by width to get actual x position
                    //round ensures no fractions clamp ensures its within valid boundaries
                    let sx = ((pos_x_px / win_w) * width as f64)
                        .round()
                        .clamp(0.0, (width - 1) as f64) as u32;
                    //(pos_y_px / win_h) gives normalized mouse y (0.0 to 1.0) * by height to get actual y position
                    //round ensures no fractions clamp ensures its within valid boundaries
                    let sy = ((pos_y_px / win_h) * height as f64)
                        .round()
                        .clamp(0.0, (height - 1) as f64) as u32;
                    //builds proper mouse packet to be sent to dispatcher
                    let packet = make_mouse_move_packet(sx, sy);
                    //sends to dispatcher
                    let _ = mouse_transmitter.send(packet);
                },
                WindowEvent::Resized(size) => {
                    //if size actually changed resize the surface and redraw the window
                    if size.width > 0 && size.height > 0 {
                            pixels.resize_surface(size.width, size.height).unwrap();
                            window.request_redraw();
                        }                },
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    //if size actually changed resize the surface and redraw the window
                    if new_inner_size.width > 0 && new_inner_size.height > 0 {
                        pixels.resize_surface(new_inner_size.width, new_inner_size.height).unwrap();
                        window.request_redraw();
                    }
                }
                _ => {}
            },
            _ => {}
        }
    });
}

fn dispatcher<T: Read + Write>(tls: &mut T, frame_transmitter: mpsc::Sender<FrameUpdate>, proxy: EventLoopProxy<UserEvent>, mouse_receiver: mpsc::Receiver<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    //create h264 decoder and buffer for frame
    let mut decoder = Decoder::new().unwrap();
    let mut h264_buffer: Vec<u8> = Vec::new();

    loop {
        //send outgoing mouse packets
        while let Ok(packet) = mouse_receiver.try_recv() {
            tls.write_all(&packet)?;
        }

        let mut header = [0u8; 5];
        //read the message header
        if let Err(e) = tls.read_exact(&mut header) {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                println!("Server disconnected");
                break Ok(());
            }
            continue;
        }

        //parse the message header into message type and payload length
        let msg_type = MessageType::from_u8(header[0]);
        let payload_len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]);
        //read the message payload
        let mut payload = vec![0u8; payload_len as usize];
        tls.read_exact(&mut payload)?;

        match msg_type {
            MessageType::FrameDelta => {
                //if there is a frame decode it
                if let Ok(Some(frame)) = decoder.decode(&payload) {
                    //get dimentions and yuv planes
                    let w = frame.width() as usize;
                    let h = frame.height() as usize;
                    let y = frame.y();
                    let u = frame.u();
                    let v = frame.v();

                    //get the strides for each plane
                    let y_stride = y.len() / h;
                    let u_stride = u.len() / ((h + 1) / 2);
                    let v_stride = v.len() / ((h + 1) / 2);

                    //convert yuv to rgba with proper strides
                    let rgba = yuv420p_to_rgba_with_stride(y, u, v, w, h, y_stride, u_stride, v_stride);

                    //send the frame to the main thread frame receiver
                    frame_transmitter.send(FrameUpdate::Full { w: w as u32, h: h as u32, bytes: rgba }).ok();
                    //prompt event loop to handle new frame
                    let _ = proxy.send_event(UserEvent::NewUpdate);
                }
            },
            MessageType::FrameEnd => {
                if h264_buffer.is_empty() {
                    continue;
                }
                //if anything is left in the buffer try to decode it
                match decoder.decode(&h264_buffer) {
                    Ok(Some(frame)) => {
                        //get dimentions and yuv planes
                        let w = frame.width() as usize;
                        let h = frame.height() as usize;
                        let y = frame.y();
                        let u = frame.u();
                        let v = frame.v();

                        //get the strides for each plane
                        let y_stride = y.len() / h;
                        let u_stride = u.len() / ((h + 1) / 2);
                        let v_stride = v.len() / ((h + 1) / 2);

                        //convert yuv to rgba with proper strides
                        let rgba = yuv420p_to_rgba_with_stride(y, u, v, w, h, y_stride, u_stride, v_stride);

                        //send the frame to the main thread frame receiver
                        frame_transmitter.send(FrameUpdate::Full { w: w as u32, h: h as u32, bytes: rgba }).ok();
                        //prompt event loop to handle new frame
                        let _ = proxy.send_event(UserEvent::NewUpdate);
                    }
                    //if no complete frame yet just wait for more data
                    Ok(None) => println!("Decoder waiting for complete frame"),
                    Err(e) => {
                        eprintln!("Decode error: {:?}", e);
                        decoder = Decoder::new().unwrap();
                    }
                }
                //clear the h264 buffer for the next frame
                h264_buffer.clear();
            },
            MessageType::FrameFull => {},
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

            MessageType::Unknown(code) => {
                println!("Unknown message type: {code:#X}, skipping {payload_len} bytes");
            }
        }
    }
}