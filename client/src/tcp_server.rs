//connect to server ip and port
//establish a socket
//exchange info
//close socket
use std::net::TcpStream;
use std::io::{Write, Read};
use std::error::Error;

pub fn run() -> Result<(), Box<dyn Error>> {
    let connection_address = "127.0.0.1:7878";
    let mut stream = TcpStream::connect(connection_address)?;
    println!("Connected to {}", connection_address);

    stream.write_all(b"hello from client")?;
    println!("Message sent");

    let mut buffer = [0; 512];
    let bytes_read = stream.read(&mut buffer)?;
    if bytes_read == 0 {
        println!("No data recieved");
    } else {
        let response = String::from_utf8_lossy(&buffer[..bytes_read]);
        println!("Server response: {}", response);

    }
    Ok(())
}