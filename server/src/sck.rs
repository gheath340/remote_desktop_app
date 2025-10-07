// use std::sync::mpsc::{self, Sender, Receiver};
// use std::sync::OnceLock;

// #[repr(C)]
// pub struct CFrame {
//     data: *const u8,
//     width: u32,
//     height: u32,
//     bytes_per_row: u32,
// }

// unsafe extern "C" {
//     fn sck_start_capture(cb: extern "C" fn(*const u8, u32, u32, u32));
// }

// // global Sender that the Objective-C callback will use
// static FRAME_TX: OnceLock<Sender<(u32, u32, Vec<u8>)>> = OnceLock::new();

// /// this function is called from Objective-C every time a new frame arrives
// extern "C" fn sck_frame_cb(ptr: *const u8, width: u32, height: u32, bytes_per_row: u32) {
//     unsafe {
//         if ptr.is_null() {
//             return;
//         }

//         let w = width as usize;
//         let h = height as usize;
//         let stride = bytes_per_row as usize;

//         // source buffer from macOS (BGRA with padding)
//         let src = std::slice::from_raw_parts(ptr, h * stride);

//         // allocate a tightly-packed RGBA buffer
//         let mut rgba = vec![0u8; w * h * 4];

//         for y in 0..h {
//             let src_row = &src[y * stride .. y * stride + w * 4];
//             let dst_row = &mut rgba[y * w * 4 .. (y + 1) * w * 4];

//             for x in 0..w {
//                 let s = x * 4;
//                 // BGRA → RGBA
//                 dst_row[s]     = src_row[s + 2];
//                 dst_row[s + 1] = src_row[s + 1];
//                 dst_row[s + 2] = src_row[s];
//                 dst_row[s + 3] = src_row[s + 3];
//             }
//         }

//         // push frame into the channel
//         if let Some(tx) = FRAME_TX.get() {
//             let _ = tx.send((width, height, rgba));
//         }
//     }
// }

// /// starts ScreenCaptureKit and returns a receiver of (width, height, rgba) frames
// pub fn start_sck_stream() -> Receiver<(u32, u32, Vec<u8>)> {
//     let (tx, rx) = mpsc::channel::<(u32, u32, Vec<u8>)>();
//     let _ = FRAME_TX.set(tx);

//     // start the native capture on a background thread
//     std::thread::spawn(|| unsafe {
//         sck_start_capture(sck_frame_cb);
//     });

//     rx
// }

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

