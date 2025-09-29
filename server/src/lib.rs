//everything that brings code together to be run by main
use std::process;
pub mod tcp_server;

pub fn run() {
    if let Err(e) = tcp_server::run() {
        eprintln!("Application error: {e}");
        process::exit(1);
    }
}