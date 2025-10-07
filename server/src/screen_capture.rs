use std::error::Error;

pub trait ScreenCapturer {
    fn capture_frame(&mut self) -> Result<(u32, u32, Vec<u8>), Box<dyn Error>>;
}

#[cfg(target_os = "macos")]
pub mod capture_macos {
    use super::ScreenCapturer;
    use core_graphics::{
        display::{CGMainDisplayID, CGDisplayBounds},
        geometry::CGRect,
        image::{CGImage, },
    };
    use core_graphics::data_provider::CGDataProvider;
    use core_foundation::{
        base::TCFType,
        data::{CFDataGetBytePtr, CFDataGetLength},
    };
    use foreign_types_shared::ForeignType;
    use std::error::Error;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGWindowListCreateImage(
            screen_bounds: CGRect,
            list_option: u32,
            window_id: u32,
            image_option: u32,
        ) -> *mut core_graphics::sys::CGImage;

        fn CGImageGetDataProvider(image: *mut core_graphics::sys::CGImage) -> *mut core_graphics::sys::CGDataProvider;
    }
    

    const KCG_WINDOW_LIST_OPTION_ALL: u32 = 0;
    const KCG_WINDOW_IMAGE_DEFAULT: u32 = 0;
    const KCG_NULL_WINDOW_ID: u32 = 0;

    pub struct MacCapturer;

    impl MacCapturer {
        pub fn new() -> Result<Self, Box<dyn Error>> {
            Ok(Self)
        }
    }

    impl ScreenCapturer for MacCapturer {
        fn capture_frame(&mut self) -> Result<(u32, u32, Vec<u8>), Box<dyn Error>> {
            let display_id = unsafe { CGMainDisplayID() };
            let bounds = unsafe { CGDisplayBounds(display_id) };

            let cg_image = unsafe {
                CGWindowListCreateImage(
                    bounds,
                    KCG_WINDOW_LIST_OPTION_ALL,
                    KCG_NULL_WINDOW_ID,
                    KCG_WINDOW_IMAGE_DEFAULT,
                )
            };
            if cg_image.is_null() {
                return Err("Failed to capture display image".into());
            }

            let image = unsafe { CGImage::from_ptr(cg_image) };
            let width = image.width();
            let height = image.height();

            let provider_ptr = unsafe { CGImageGetDataProvider(image.as_ptr()) };
            if provider_ptr.is_null() {
                return Err("Missing data provider".into());
            }

            // ✅ get data provider (modern API)
            let provider: CGDataProvider = unsafe {
                CGDataProvider::from_ptr(provider_ptr)
            };
            let cfdata = unsafe { provider.copy_data() };
            let len = unsafe { CFDataGetLength(cfdata.as_concrete_TypeRef()) } as usize;
            let ptr = unsafe { CFDataGetBytePtr(cfdata.as_concrete_TypeRef()) };
            let bytes = unsafe { std::slice::from_raw_parts(ptr, len).to_vec() };

            let stride = len / height;
            let row_bytes = width * 4;

            let mut rgba = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                let row_start = y * stride;
                let row = &bytes[row_start .. row_start + row_bytes];
                for chunk in row.chunks_exact(4) {
                    rgba.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
                }
            }

            // ✅ convert BGRA → RGBA
            // let mut rgba = Vec::with_capacity(width * height * 4);
            // for chunk in bytes.chunks_exact(4) {
            //     rgba.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
            // }

            Ok((width as u32, height as u32, rgba))
        }
    }
}
#[cfg(target_os = "macos")]
pub use capture_macos::MacCapturer as ActiveCapturer;

#[cfg(not(target_os = "macos"))]
pub use capture_stub::StubCapturer as ActiveCapturer;
// Dummy fallback (for non-mac builds, like Linux tests)
#[cfg(not(target_os = "macos"))]
pub mod capture_stub {
    use super::ScreenCapturer;
    use std::error::Error;

    pub struct StubCapturer;

    impl StubCapturer {
        pub fn new() -> Result<Self, Box<dyn Error>> {
            Ok(Self)
        }
    }

    impl ScreenCapturer for StubCapturer {
        fn capture_frame(&mut self) -> Result<(u32, u32, Vec<u8>), Box<dyn Error>> {
            Err("Screen capture not supported on this platform".into())
        }
    }
}