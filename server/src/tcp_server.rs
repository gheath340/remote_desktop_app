use std::net::{ TcpListener, TcpStream };
use std::io::{ Read, Write, ErrorKind };
use std::error::Error;
use std::sync::Arc;
use rustls::{ ServerConfig, ServerConnection, Stream };
use common::message_type::MessageType;
use crate::message_type_handlers;


fn handel_client(mut tcp: TcpStream, tls_config: Arc<ServerConfig>) -> Result <(), Box<dyn Error>> {
    //create TLS server machine then create TLS stream
    let mut tls_conn = ServerConnection::new(tls_config.clone())?;
    let mut tls = Stream::new(&mut tls_conn, &mut tcp);

    println!("New connection: {:#?}", tls);

    //send first frame right on connection
    message_type_handlers::handle_frame_full(&mut tls)?;


    //read message from client
    dispatcher(&mut tls)?;
    
    //send response to client
    // let payload = b"Hello, server";
    // send_response(&mut tls, MessageType::Text, payload)?;
    // println!("Response sent!");

    Ok(())
}

pub fn run(tls_config: Arc<ServerConfig>) -> Result<(), Box<dyn Error>> {
    //create tcp listener
    let listener = TcpListener::bind("127.0.0.1:7878")?;
    println!("Tcp server listening to port 127.0.0.1:7878");
    //call handel_client on all clients that contact tcp adress
    for stream in listener.incoming() {
        handel_client(stream?, tls_config.clone())?;
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

            MessageType::FrameFull => message_type_handlers::handle_frame_full(tls)?,
            MessageType::FrameDelta => message_type_handlers::handle_frame_delta(&payload)?,
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
    //get the byte value from msg_type
    let type_byte = msg_type.to_u8();

    //encode the length of payload into bytes
    let len_bytes = (payload.len() as u32).to_be_bytes();

    //write the header
    stream.write_all(&[type_byte])?;
    stream.write_all(&len_bytes)?;

    //write payload and make sure it goes
    stream.write_all(payload)?;
    stream.flush()?;

    Ok(())
}
