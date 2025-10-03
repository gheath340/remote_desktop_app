//everything that brings code together to be run by main
use std::error::Error;

pub mod tcp_server;
mod client_tls;
mod message_type_handlers;

//load client tls config and run server
pub fn run() -> Result<(), Box<dyn Error>> {
    let cfg = client_tls::load_client_config()?;
    tcp_server::run(cfg)
}