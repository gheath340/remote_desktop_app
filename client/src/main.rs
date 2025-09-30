//only main function with the very limited, necessary to run code
use client;
use std::process;

fn main() {
    if let Err(e) = client::run() {
        eprintln!("Application error: {e}");
        process::exit(1);
    }
}
