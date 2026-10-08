    // objc 0.2 macros expand to a `cfg(feature = "cargo-clippy")` probe that
    // newer rustc flags; harmless upstream noise (same as glass.rs).
    #![allow(unexpected_cfgs)]

    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    /// Pure focus of an already-running Hermes — no relaunch, no reopen
    /// event, no new window. NSRunningApplication is safe off the main
    /// thread (no view work here).
    pub fn focus_running_hermes() -> bool {
        unsafe {
            let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
            if workspace.is_null() {
                return false;
            }
            let apps: *mut Object = msg_send![workspace, runningApplications];
            if apps.is_null() {
                return false;
            }
            let count: usize = msg_send![apps, count];
            for i in 0..count {
                let app: *mut Object = msg_send![apps, objectAtIndex: i];
                if app.is_null() {
                    continue;
                }
                let name: *mut Object = msg_send![app, localizedName];
                if name.is_null() {
                    continue;
                }
                let utf8: *const std::os::raw::c_char = msg_send![name, UTF8String];
                if utf8.is_null() {
                    continue;
                }
                if std::ffi::CStr::from_ptr(utf8)
                    .to_string_lossy()
                    .eq_ignore_ascii_case("hermes")
                {
                    // NSApplicationActivateAllWindows | IgnoringOtherApps.
                    let ok: bool = msg_send![app, activateWithOptions: 3usize];
                    return ok;
                }
            }
            false
        }
    }
