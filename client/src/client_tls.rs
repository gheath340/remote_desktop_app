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
    //Read servers certs and puts them into a vec 
    //let mut cert_reader = BufReader::new(File::open("../certs/server.crt")?);
    // let certs_vec: Vec<CertificateDer<'static>> = certs(&mut cert_reader)
    //     .collect::<Result<_,_>>()?;
    // //Add server cert into RootCertStore
    // let mut roots = rustls::RootCertStore::empty();
    // for cert in certs_vec {
    //     roots.add(cert)?;
    // }
    // //Create client config
    // let cfg = ClientConfig::builder()
    //     .with_root_certificates(roots)
    //     .with_no_client_auth();
    // //Return Arc of cfg if successful
    // Ok(Arc::new(cfg))
    let ca_file = &mut BufReader::new(File::open("../certs/ca.crt").expect("missing ca.crt"));
    let ca_certs: Vec<CertificateDer<'static>> = certs(ca_file)
        .collect::<Result<_,_>>()?;

    let mut roots = RootCertStore::empty();
    for cert in ca_certs {
        roots.add(cert)?;
    }

    let cfg = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    Ok(Arc::new(cfg))
}