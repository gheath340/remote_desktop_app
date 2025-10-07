//only main function with the very limited, necessary to run code
use std::process;
use server;
use server::screen_capture::ScreenCapturer;

fn main() {
    //call lib.rs run
    if let Err(e) = server::run() {
        eprintln!("Application error: {e}");
        process::exit(1);
    }
}

// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let mut capturer = server::screen_capture::capture_macos::MacCapturer::new()?;
//     let (w, h, rgba) = capturer.capture_frame()?;
//     println!("Captured {}×{} ({} bytes)", w, h, rgba.len());
//     Ok(())
// }