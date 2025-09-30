//needs to reserve an ip and port
//listen for incoming connections on port
//accept a client connection(OS gives new socket)
//read from and write to socket
//close connection
use std::net::{ TcpListener, TcpStream };
use std::io::{ Read, Write };
use std::error::Error;
use std::sync::Arc;
use rustls::{ ServerConfig, ServerConnection, Stream };


fn handel_client(mut tcp: TcpStream, tls_config: Arc<ServerConfig>) -> Result <(), Box<dyn Error>> {
    //create TLS server machine then create TLS stream
    let mut tls_conn = ServerConnection::new(tls_config.clone())?;
    let mut tls = Stream::new(&mut tls_conn, &mut tcp);

    println!("New connection: {:#?}", tls);
    //read message from client
    let mut buffer = [0; 512];
    let bytes_read = tls.read(&mut buffer)?;
    let message = String::from_utf8_lossy(&buffer[..bytes_read]);
    println!("TLS secured client message: {}", message);
    //send response to client
    tls.write_all(b"hello from server")?;
    println!("Response sent");

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

fn parse_input() {
    //create header and read data into header
    let mut header = [0u8; 5];
    tls.read_exact(&mut header);
    //parse message type and payload_len from header
    let msg_type = MessageType::from_u8(header[0]);
    let payload_len = u32::from_be_bytes(header[1], header[2], header[3], header[4]);
    //create empty vec that is the appropriate length for payload and fill it wih payload
    let mut payload = vec![0u8; payload_len as usize];
    tls.read_exact(&mut payload);
}

//enum for all different message types
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MessageType {
    // Session
    Text        = 0x01,
    Connect     = 0x02,
    Disconnect  = 0x03,
    Error       = 0x04,

    // Display / Frames
    FrameFull   = 0x10,
    FrameDelta  = 0x11,
    CursorShape = 0x12,
    CursorPos   = 0x13,
    Resize      = 0x14,

    // Input
    KeyDown     = 0x20,
    KeyUp       = 0x21,
    MouseMove   = 0x22,
    MouseDown   = 0x23,
    MouseUp     = 0x24,
    MouseScroll = 0x25,

    // Clipboard
    Clipboard   = 0x30,

    //Catch all others
    Unknown(u8);,
}

impl MessageType {
    //take in u8 and return the correct message type
    pn fn from_u8(v: u8) -> Self {
        match v {
            0x01 => MessageType::Text,
            0x02 => MessageType::Connect,
            0x03 => MessageType::Disconnect,
            0x04 => MessageType::Error,

            0x10 => MessageType::FrameFull,
            0x11 => MessageType::FrameDelta,
            0x12 => MessageType::CursorShape,
            0x13 => MessageType::CursorPos,
            0x14 => MessageType::Resize,

            0x20 => MessageType::KeyDown,
            0x21 => MessageType::KeyUp,
            0x22 => MessageType::MouseMove,
            0x23 => MessageType::MouseDown,
            0x24 => MessageType::MouseUp,
            0x25 => MessageType::MouseScroll,

            0x30 => MessageType::Clipboard,

            other => MessageType::Unknown(other),
        }
    }
}

