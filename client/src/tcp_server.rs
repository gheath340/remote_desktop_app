use common::message_type::MessageType;
use std::{ 
    process,
    net::TcpStream,
    io::{ Write, Read, Cursor },
    error::Error,
    sync::{ Arc, mpsc },
    time::{ Instant, Duration },
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
use image::io::Reader as ImageReader;
use image::DynamicImage;
use turbojpeg::{Decompressor, Image, PixelFormat};


 #[derive(Debug)]
pub enum UserEvent {
    NewUpdate,
    Redraw,
}

pub enum FrameUpdate {
    Full(Vec<u8>),
    Delta(Vec<u8>),
}

pub fn run(tls_config: Arc<ClientConfig>) -> Result<(), Box<dyn Error>> {
    //temp connection address
    let connection_address = "127.0.0.1:7878";

    //create the UI, main thread loop
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    //the proxy that allows dispatcher thread to send message to event_loop
    let proxy = event_loop.create_proxy();

    //create the transmitter and reciever for the mpsc channel(message queue) that carries messages of the type FrameUpdate
    let (channel_transmitter, channel_reciever) = mpsc::channel::<FrameUpdate>();

    //create thread for dispatcher
    std::thread::spawn(move || {
        //create tcp connection
        let mut tcp = TcpStream::connect(connection_address)
            .unwrap_or_else(|e| {
                eprintln!("Failed to create TCP connection: {e}");
                process::exit(1);
            });
        println!("Connected to {} via TCP", connection_address);

        //get hostname of server
        let server_name = ServerName::try_from("localhost")
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
    let first_full_bytes = loop {
        match channel_reciever.recv()? {
            FrameUpdate::Full(bytes) => break bytes,
            FrameUpdate::Delta(_) => {
                continue;
            }
        }
    };

    //get image dimensions and image data
    let width = u32::from_be_bytes(first_full_bytes[0..4].try_into().unwrap());
    let height = u32::from_be_bytes(first_full_bytes[4..8].try_into().unwrap());

    //build window
    let window = WindowBuilder::new()
        .with_title("Remote desktop client")
        .build(&event_loop)?;

    //create surface and attatch pixels to it
    let surface_texture = SurfaceTexture::new(width, height, &window);
    let mut pixels = Pixels::new(width, height, surface_texture)?;

    //put image into pixels to display on window
    {
        message_type_handlers::handle_frame_full(&first_full_bytes, &mut pixels)?;
        pixels.render().unwrap();
    }

    let mut last_frame = Instant::now();
    let mut frame_count = 0u32;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(UserEvent::NewUpdate) => {
                let mut got_any = false;
                while let Ok(update) = channel_reciever.try_recv() {
                    match update {
                        FrameUpdate::Full(bytes) => {
                            if let Err(e) = message_type_handlers::handle_frame_full(&bytes, &mut pixels) {
                                eprintln!("Frame full error: {e}");
                            }
                            got_any = true;
                        }
                        //I dont want handle_frame_delta to actually change the window, i want it to hold the updates until FrameEnd is called
                        FrameUpdate::Delta(bytes) => {
                            if let Err(e) = message_type_handlers::handle_frame_delta(&bytes, &mut pixels) {
                                eprintln!("Frame delta error: {e}");
                            }
                            got_any = true;
                        }
                    }
                }
                if got_any {
                    window.request_redraw();
                }
            },
            //call window request redraw for consistency
            Event::UserEvent(UserEvent::Redraw) => {
                window.request_redraw();
            }
            //window.request_redraw() calls this
            Event::RedrawRequested(_) => {
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
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => {}
            },
            _ => {}
        }
    });
}

fn dispatcher<T: Read + Write>(tls: &mut T, channel_transmitter: mpsc::Sender<FrameUpdate>, proxy: EventLoopProxy<UserEvent>) -> Result<(), Box<dyn Error>> {
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
                let start = Instant::now();

                let header = decompressor.read_header(&payload)?;
                let w = header.width as usize;
                let h = header.height as usize;

                let mut rgba = vec![0u8; w * h * 4];
                let mut out = turbojpeg::Image {
                    pixels: rgba.as_mut_slice(),
                    width: w,
                    pitch: w * 4,
                    height: h,
                    format: turbojpeg::PixelFormat::RGBA,
                };
                decompressor.decompress(&payload, out)?;

                // re-wrap for your existing handler
                let mut framed = Vec::with_capacity(8 + rgba.len());
                framed.extend_from_slice(&(w as u32).to_be_bytes());
                framed.extend_from_slice(&(h as u32).to_be_bytes());
                framed.extend_from_slice(&rgba);

                channel_transmitter.send(FrameUpdate::Full(framed)).ok();
                let _ = proxy.send_event(UserEvent::NewUpdate);
            },
            MessageType::FrameDelta => {
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