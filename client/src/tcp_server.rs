use common::message_type::MessageType;
use std::{ 
    process,
    net::TcpStream,
    io::{ Write, Read },
    error::Error,
    sync::{ Arc, Mutex },
};
use rustls::{ 
    ClientConfig, 
    ClientConnection, 
    Stream, 
    pki_types::ServerName,
 };
use crate::{ 
    window_init::window_init, 
    message_type_handlers,
 };


pub type SharedFrame = Arc<Mutex<Option<Vec<u8>>>>;

pub fn run(tls_config: Arc<ClientConfig>) -> Result<(), Box<dyn Error>> {
    //temp connection address
    let connection_address = "127.0.0.1:7878";
    
    //create SharedFrame
    let shared_frame: SharedFrame = Arc::new(Mutex::new(None));

    //shared frame clone for dispatcher thread
    let sf_clone = shared_frame.clone();

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

        if let Err(e) = dispatcher(&mut tls, sf_clone) {
            eprintln!("Dispatcher error: {e}");
        }
    });

    //wait to call window_init till first frame is in shared_frame
    loop {
        if shared_frame.lock().unwrap().is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    //initialize window
    window_init(shared_frame)?;

    Ok(())
}

//sends given message to server
// fn send_message<T: Write>(stream: &mut T, msg_type: MessageType, payload: &[u8]) -> Result<(), Box<dyn Error>> {
//     //get the byte value from msg_type
//     let type_byte = msg_type.to_u8();

//     //encode the length of payload into bytes
//     let len_bytes = (payload.len() as u32).to_be_bytes();

//     //write the header
//     stream.write_all(&[type_byte])?;
//     stream.write_all(&len_bytes)?;

//     //write payload and make sure it goes
//     stream.write_all(payload)?;
//     stream.flush()?;

//     Ok(())
// }

//takes info from server and dispatches to correct MessageType handler
fn dispatcher<T: Read + Write>(tls: &mut T, shared_frame: SharedFrame) -> Result<(), Box<dyn Error>> {
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
            //MessageType::FrameFull => handle_frame_full(&payload)?,
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
                let mut guard = shared_frame.lock().unwrap();
                *guard = Some(payload);
            },
            MessageType::FrameDelta => message_type_handlers::handle_frame_delta(&payload)?,

            MessageType::Unknown(code) => {
                println!("Unknown message type: {code:#X}, skipping {payload_len} bytes");
            }
        }
    }
}