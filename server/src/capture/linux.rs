use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use xcap::Monitor;

pub fn start_sck_stream() -> Receiver<(usize, usize, Vec<u8>)> {
    let (tx, rx) = channel();

    std::thread::spawn(move || {
        // Get all monitors
        let monitors = Monitor::all().expect("Failed to query monitors");
        let monitor = monitors.first().expect("No monitors found");

        loop {
            match monitor.capture_image() {
                Ok(img) => {
                    let width = img.width() as usize;
                    let height = img.height() as usize;

                    // Convert image to RGBA8
                    let rgba = img.into_raw();

                    let _ = tx.send((width, height, rgba));
                }
                Err(err) => {
                    eprintln!("Capture error: {:?}", err);
                    std::thread::sleep(Duration::from_millis(16));
                }
            }
        }
    });

    rx
}

