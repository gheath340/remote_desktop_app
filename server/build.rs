fn main() {
    #[cfg(target_os = "macos")]
    {
        let mut build = cc::Build::new();
        build.file("src/maccapture.m")
             .flag("-fobjc-arc") // enable Objective-C Automatic Reference Counting
             .compile("maccapture");

        // link Apple frameworks required by ScreenCaptureKit
        println!("cargo:rustc-link-lib=framework=ScreenCaptureKit");
        println!("cargo:rustc-link-lib=framework=CoreVideo");
        println!("cargo:rustc-link-lib=framework=CoreMedia");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
        println!("cargo:rustc-link-lib=framework=Foundation");

        // tell Cargo to rebuild if these files change
        println!("cargo:rerun-if-changed=src/maccapture.m");
        println!("cargo:rerun-if-changed=src/maccapture.h");

        println!("cargo:rustc-link-lib=framework=Accelerate");
    }
}
