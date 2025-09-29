//only main function with the very limited, necessary to run code
use server;

fn main() {
    if let Err(e) = server::run() {
        eprintln!("Application error: {e}");
        process::exit(1);
    }
}
