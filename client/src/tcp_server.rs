//connect to server ip and port
//establish a socket
//exchange info
//close socket
use std::net::TcpStream;
use std::io::{Write, Read};
use std::error::Error;
use std::sync::Arc;
use rustls::{ClientConfig, ClientConnection, Stream};
use rustls::pki_types::ServerName;
use common::message_type::MessageType;

pub fn run(tls_config: Arc<ClientConfig>) -> Result<(), Box<dyn Error>> {
    //temp variable
    let connection_address = "127.0.0.1:7878";

    //connect to server address
    let mut tcp = TcpStream::connect(connection_address)?;
    println!("Connected to {} via TCP", connection_address);

    //get hostname of server, create TLS client state machine, create a TLS stream
    let server_name = ServerName::try_from("localhost")?;
    let mut tls_connection = ClientConnection::new(tls_config, server_name)?;
    let mut tls = Stream::new(&mut tls_connection, &mut tcp);

    //send message to server
    let payload = b"Hello, server";
    send_message(&mut tls, MessageType::Text, payload)?;
    println!("Message sent!");
    tls.conn.send_close_notify();
    tls.flush()?;

     // //read response from server
    // let mut buffer = [0; 512];
    // let bytes_read = tls.read(&mut buffer)?;
    // let response = String::from_utf8_lossy(&buffer[..bytes_read]);
    // println!("Server response: {}", response);

    Ok(())
}

//sends given message to server
fn send_message<T: Write>(stream: &mut T, msg_type: MessageType, payload: &[u8]) -> Result<(), Box<dyn Error>> {
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