fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=src/talk_mac.m");
        println!("cargo:rerun-if-changed=src/objc_try.m");
        cc::Build::new()
            .file("src/talk_mac.m")
            .flag("-fobjc-arc")
            .compile("orbitty_talk_mac");
        cc::Build::new()
            .file("src/objc_try.m")
            .flag("-fobjc-arc")
            .compile("orbitty_objc_try");
        println!("cargo:rustc-link-lib=framework=Speech");
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
    tauri_build::build();
}
