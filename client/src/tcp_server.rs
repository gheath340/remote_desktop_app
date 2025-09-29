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

pub fn run(tls_config: Arc<ClientConfig>) -> Result<(), Box<dyn Error>> {
    let connection_address = "127.0.0.1:7878";

    let mut tcp = TcpStream::connect(connection_address)?;
    println!("Connected to {} via TCP", connection_address);

    let server_name = ServerName::try_from("localhost")?;
    let mut tls_connection = ClientConnection::new(tls_config, server_name)?;
    let mut tls = Stream::new(&mut tls_connection, &mut tcp);

    tls.write_all(b"hello from client")?;
    println!("Message sent");

    let mut buffer = [0; 512];
    let bytes_read = tls.read(&mut buffer)?;
    let response = String::from_utf8_lossy(&buffer[..bytes_read]);
    println!("Server response: {}", response);

    Ok(())
}