use std::sync::{
    mpsc::{channel, Sender, Receiver},
    Mutex, LazyLock,
};
use std::os::raw::{c_uint, c_uchar};

// Safe global sender storage
static FRAME_SENDER: LazyLock<Mutex<Option<Sender<(usize, usize, Vec<u8>)>>>> =
    LazyLock::new(|| Mutex::new(None));

unsafe extern "C" {
    fn sck_start_capture(cb: extern "C" fn(*const c_uchar, c_uint, c_uint, c_uint));
}

// This callback is invoked from Objective-C each time a new frame is ready
extern "C" fn sck_frame_cb(
    data: *const c_uchar,
    width: c_uint,
    height: c_uint,
    bytes_per_row: c_uint,
) {
    if data.is_null() {
        return;
    }

    // ScreenCaptureKit adds alignment padding at the end of each row.
    // We must copy only width*4 bytes per row to remove the padding.
    let width = width as usize;
    let height = height as usize;
    let bytes_per_row = bytes_per_row as usize;

    let mut rgba = Vec::with_capacity(width * height * 4);

    unsafe {
        for y in 0..height {
            let row_start = data.add(y * bytes_per_row);
            let row_slice = std::slice::from_raw_parts(row_start, width * 4);
            rgba.extend_from_slice(row_slice);
        }
    }

    // Send to main server loop
    let guard = FRAME_SENDER.lock().unwrap();
    if let Some(sender) = &*guard {
        let _ = sender.send((width, height, rgba));
    }
}

pub fn start_sck_stream() -> Receiver<(usize, usize, Vec<u8>)> {
    let (tx, rx) = channel();
    *FRAME_SENDER.lock().unwrap() = Some(tx);
    unsafe { sck_start_capture(sck_frame_cb) };
    rx
}

