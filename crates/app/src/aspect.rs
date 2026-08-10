//! Keeping the window at the proportions the surface wants.

/// Asks the window server to hold the window at `width : height`.
///
/// Returns whether the platform enforces the ratio itself. When it does not, the caller has to
/// correct the window after each resize instead.
#[cfg(target_os = "macos")]
pub fn enforce(frame: &eframe::Frame, width: f32, height: f32) -> bool {
    use objc2_app_kit::NSView;
    use objc2_foundation::NSSize;
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};

    let Ok(handle) = frame.window_handle() else {
        return false;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return false;
    };
    // eframe hands out a live NSView for as long as the window is open
    #[allow(unsafe_code)]
    let view: &NSView = unsafe { appkit.ns_view.cast::<NSView>().as_ref() };
    let Some(window) = view.window() else {
        return false;
    };
    window.setContentAspectRatio(NSSize::new(f64::from(width), f64::from(height)));
    true
}

/// Asks the window server to hold the window at `width : height`.
///
/// Returns whether the platform enforces the ratio itself. When it does not, the caller has to
/// correct the window after each resize instead.
#[cfg(not(target_os = "macos"))]
pub fn enforce(frame: &eframe::Frame, width: f32, height: f32) -> bool {
    let _ = (frame, width, height);
    false
}
