//needs to reserve an ip and port
//listen for incoming connections on port
//accept a client connection(OS gives new socket)
//read from and write to socket
//close connection
use std::net::{ TcpListener, TcpStream };
use std::io::{Read, Write};
use std::error::Error;

fn handel_client(mut connection: TcpStream) -> Result <(), Box<dyn Error>> {
    println!("New connection: {:#?}", connection);

    let mut buffer = [0; 512];
    let bytes_read = connection.read(&mut buffer)?;
    let message = String::from_utf8_lossy(&buffer[..bytes_read]);
    println!("Client message: {}", message);

    connection.write_all(b"hello from server")?;
    println!("Response sent");

    Ok(())
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:7878")?;
    println!("Tcp server listening to port 127.0.0.1:7878");

    for stream in listener.incoming() {
        handel_client(stream?)?;
    }
    Ok(())
}