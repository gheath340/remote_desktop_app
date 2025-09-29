use std::{fs::File, io::BufReader, sync::Arc};
use rustls::{ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::error::Error;

pub fn load_server_config() -> Result<Arc<ServerConfig>, Box<dyn Error>> {
    let cert_file = &mut BufReader::new(File::open("certs/server.crt")?);
    let key_file = &mut BufReader::new(File::open("certs/server.key")?);

    let cert_chain: Vec<CertificateDer<'static>> = certs(cert_file)
        .collect::<Result<_,_>>()?;

    let mut keys: Vec<PrivateKeyDer<'static>> = pkcs8_private_keys(key_file)
        .map(|res| res.map(PrivateKeyDer::from))
        .collect::<Result<_,_>>()?;
    let key = keys.remove(0);

    let cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)?;

    Ok(Arc::new(cfg))
}