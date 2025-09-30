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