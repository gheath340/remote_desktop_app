use std::{
    fs::File, 
    io::{ BufReader }, 
    sync::Arc,
    error::Error,
};
use rustls::{ ClientConfig, RootCertStore, };
use rustls_pemfile::certs;
use rustls::pki_types::{ CertificateDer, };

pub fn load_client_config() -> Result<Arc<ClientConfig>, Box<dyn Error>> {
    //get ca file
    let ca_file = &mut BufReader::new(File::open("../certs/ca.crt").expect("missing ca.crt"));
    //get vec of certs from ca
    let ca_certs: Vec<CertificateDer<'static>> = certs(ca_file)
        .collect::<Result<_,_>>()?;

    //add root for each cert in ca_certs
    let mut roots = RootCertStore::empty();
    for cert in ca_certs {
        roots.add(cert)?;
    }

    //build config
    let cfg = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    Ok(Arc::new(cfg))
}