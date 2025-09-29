use std::{fs::File, io::{BufReader, Read, Write}, net::TcpStream, sync::Arc};
use rustls::{ClientConfig, ClientConnection, Stream};
use rustls_pemfile::certs;
use rustls::pki_types::{CertificateDer, ServerName};

pub fn load_client_config() -> Result<Arc<ClientConfig>, Box<dyn Error>> {
    
}