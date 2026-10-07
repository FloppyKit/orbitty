fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=src/talk_mac.m");
        println!("cargo:rerun-if-changed=src/objc_try.m");
        println!("cargo:rerun-if-changed=src/attach_mac.m");
        println!("cargo:rerun-if-changed=src/window_thumb_mac.m");
        cc::Build::new()
            .file("src/talk_mac.m")
            .flag("-fobjc-arc")
            .compile("orbitty_talk_mac");
        cc::Build::new()
            .file("src/objc_try.m")
            .flag("-fobjc-arc")
            .compile("orbitty_objc_try");
        // one-file NSOpenPanel for the composer chip.
        cc::Build::new()
            .file("src/attach_mac.m")
            .flag("-fobjc-arc")
            .compile("orbitty_attach_mac");
        cc::Build::new()
            .file("src/window_thumb_mac.m")
            .flag("-fobjc-arc")
            .compile("orbitty_window_thumb");
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,ScreenCaptureKit");
        println!("cargo:rustc-link-lib=framework=Speech");
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=AppKit");
    }
    tauri_build::build();
}
