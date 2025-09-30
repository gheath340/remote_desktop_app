use std::{fs::File, io::{BufReader, Read, Write}, net::TcpStream, sync::Arc};
use rustls::{ClientConfig, ClientConnection, Stream};
use rustls_pemfile::certs;
use rustls::pki_types::{CertificateDer, ServerName};
use std::error::Error;

pub fn load_client_config() -> Result<Arc<ClientConfig>, Box<dyn Error>> {
    //Read servers cert
    let mut cert_reader = BufReader::new(File::open("../certs/server.crt")?);
    let certs_vec: Vec<CertificateDer<'static>> = certs(&mut cert_reader)
        .collect::<Result<_,_>>()?;
    //Add server cert into RootCertStore
    let mut roots = rustls::RootCertStore::empty();
    for cert in certs_vec {
        roots.add(cert)?;
    }
    //Create client config
    let cfg = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    //Return Arc of cfg if successful
    Ok(Arc::new(cfg))
}