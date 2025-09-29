//everything that brings code together to be run by main
use std::process;
use std::erro::Error;

mod tcp_server;
mod tls;

pub fn run() -> Result<(), Box<dyn Error>> {
    let cfg = load_server_config()?;
    tcp_server::run(cfg);
}