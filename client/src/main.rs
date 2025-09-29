//only main function with the very limited, necessary to run code
use client;

fn main() {
    if let Err(e) client::run() {
        eprintln!("Application error: {e}");
        process::exit(1);
    }
}
