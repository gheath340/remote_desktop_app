//everything that brings code together to be run by main
use std::error::Error;

pub mod screen_capture;
mod message_type_handlers;
mod tcp_server;
mod tls;
//load tls config and call tcp_server run
pub fn run() -> Result<(), Box<dyn Error>> {
    let cfg = tls::load_server_config()?;
    tcp_server::run(cfg)
}