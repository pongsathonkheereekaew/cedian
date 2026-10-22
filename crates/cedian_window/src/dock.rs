//! The Dock icon's badge: how many threads need the person, across the app (Interface,
//! Notifications). Badges only; banners wait until the rest of the app is done.

/// Shows `count` on the Dock icon, or clears it at zero.
#[cfg(target_os = "macos")]
pub fn set_badge(count: usize) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::NSString;

    let Some(main) = MainThreadMarker::new() else {
        log::warn!("the Dock badge can only be set from the main thread");
        return;
    };
    let label = (count > 0).then(|| NSString::from_str(&count.to_string()));
    NSApplication::sharedApplication(main)
        .dockTile()
        .setBadgeLabel(label.as_deref());
}

/// Other platforms have no Dock; the window title carries the count there.
#[cfg(not(target_os = "macos"))]
pub fn set_badge(_count: usize) {}
