//! Desktop notifications with click-to-open, the Dock badge, and bringing the
//! app to the front.
//!
//! - **macOS**: UserNotifications (`UNUserNotificationCenter`). A click,
//!   including one that launches the app, calls the `install` callback with
//!   the notification's id. UserNotifications aborts the process when there is
//!   no bundle identity, so every call is guarded: a plain `cargo run` binary
//!   (not inside a `.app`) runs without notifications.
//! - **Elsewhere**: no-ops, unless the `fallback` feature shows notifications
//!   through notify-rust (show-only: no click routing, withdraw or badge).
//!
//! Ids are the caller's: encode whatever a click needs to find its target
//! again (yapper uses `"{account}:{message_id}"`). Diagnostics go to the `log`
//! facade.
//!
//! Extracted from yapper (`crates/yapper-ui/src/platform.rs`).

#[cfg(target_os = "macos")]
pub use mac::*;

#[cfg(not(target_os = "macos"))]
pub use other::*;

#[cfg(not(target_os = "macos"))]
mod other {
    /// Remembers `app_name` for the `fallback` feature; clicks are not routed
    /// off macOS.
    pub fn install(app_name: &str, _on_click: impl Fn(String) + Send + Sync + 'static) {
        #[cfg(feature = "fallback")]
        {
            let _ = APP_NAME.set(app_name.to_owned());
        }
        #[cfg(not(feature = "fallback"))]
        let _ = app_name;
    }

    #[cfg(feature = "fallback")]
    static APP_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();

    /// Whether notifications can be shown at all.
    pub fn available() -> bool {
        cfg!(feature = "fallback")
    }

    pub fn notify(_id: &str, _thread: &str, title: &str, subtitle: &str, body: &str) {
        #[cfg(feature = "fallback")]
        {
            let text = if subtitle.is_empty() {
                body.to_owned()
            } else {
                format!("{subtitle}\n{body}")
            };
            let mut n = notify_rust::Notification::new();
            n.summary(title).body(&text);
            if let Some(name) = APP_NAME.get() {
                n.appname(name);
            }
            if let Err(e) = n.show() {
                log::warn!("notifications: post failed: {e}");
            }
        }
        #[cfg(not(feature = "fallback"))]
        let _ = (title, subtitle, body);
    }

    pub fn withdraw(_ids: &[String]) {}

    pub fn set_badge(_count: usize) {}

    pub fn activate() {}

    pub fn is_active() -> bool {
        true
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::sync::OnceLock;

    use block2::{DynBlock, RcBlock};
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, NSObject, ProtocolObject};
    use objc2::{AnyThread, MainThreadMarker, define_class, msg_send};
    use objc2_app_kit::NSApplication;
    use objc2_foundation::{NSArray, NSBundle, NSError, NSObjectProtocol, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
        UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
        UNNotificationSound, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };

    type OnClick = Box<dyn Fn(String) + Send + Sync>;
    static ON_CLICK: OnceLock<OnClick> = OnceLock::new();

    /// Running from an app bundle, so UserNotifications can be used.
    fn bundled() -> bool {
        static BUNDLED: OnceLock<bool> = OnceLock::new();
        *BUNDLED.get_or_init(|| {
            let b = NSBundle::mainBundle();
            b.bundleIdentifier().is_some() && b.bundlePath().to_string().ends_with(".app")
        })
    }

    define_class!(
        // Not main-thread-only: which thread calls the delegate is not
        // documented, so it only extracts plain data and hands over.
        #[unsafe(super(NSObject))]
        #[name = "SlintKitNotificationDelegate"]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl UNUserNotificationCenterDelegate for Delegate {
            // Without this nothing shows while the app is frontmost; the
            // app itself decides not to post then.
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(
                &self,
                _center: &UNUserNotificationCenter,
                _n: &UNNotification,
                done: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
            ) {
                done.call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List
                    | UNNotificationPresentationOptions::Sound,));
            }

            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(
                &self,
                _center: &UNUserNotificationCenter,
                response: &UNNotificationResponse,
                done: &DynBlock<dyn Fn()>,
            ) {
                let id = response.notification().request().identifier().to_string();
                done.call(());
                if let Some(f) = ON_CLICK.get() {
                    f(id);
                }
            }
        }
    );

    /// Installs the click handler and asks for permission. Call once, after
    /// the app's window exists and before its event loop runs: a click that
    /// launches the app is delivered to the delegate set here. `on_click`
    /// gets the notification id and runs on an arbitrary thread. `app_name`
    /// is only used off macOS (the bundle names the app here).
    pub fn install(_app_name: &str, on_click: impl Fn(String) + Send + Sync + 'static) {
        if !bundled() {
            log::info!("notifications: not running from an app bundle, disabled");
            return;
        }
        let _ = ON_CLICK.set(Box::new(on_click));
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let delegate: Retained<Delegate> = unsafe { msg_send![Delegate::alloc(), init] };
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // The delegate property is weak; the delegate lives as long as the
        // process.
        std::mem::forget(delegate);
        // Badge must be asked for up front: added later it starts off.
        let done = RcBlock::new(|granted: Bool, err: *mut NSError| {
            let err = unsafe { err.as_ref() }.map(|e| e.localizedDescription().to_string());
            log::info!(
                "notifications: permission {}{}",
                if granted.as_bool() {
                    "granted"
                } else {
                    "not granted"
                },
                err.map(|e| format!(" ({e})")).unwrap_or_default()
            );
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert
                | UNAuthorizationOptions::Sound
                | UNAuthorizationOptions::Badge,
            &done,
        );
    }

    /// Whether notifications can be shown at all (running from a bundle).
    pub fn available() -> bool {
        bundled()
    }

    /// Posts a notification; a later one with the same id replaces it.
    /// `thread` groups related notifications (e.g. one conversation).
    pub fn notify(id: &str, thread: &str, title: &str, subtitle: &str, body: &str) {
        if !bundled() {
            return;
        }
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        if !subtitle.is_empty() {
            content.setSubtitle(&NSString::from_str(subtitle));
        }
        content.setBody(&NSString::from_str(body));
        content.setThreadIdentifier(&NSString::from_str(thread));
        content.setSound(Some(&UNNotificationSound::defaultSound()));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(id),
            &content,
            None,
        );
        let done = RcBlock::new(|err: *mut NSError| {
            if let Some(e) = unsafe { err.as_ref() } {
                log::warn!("notifications: post failed: {}", e.localizedDescription());
            }
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .addNotificationRequest_withCompletionHandler(&request, Some(&done));
    }

    /// Removes delivered notifications, e.g. once their target was seen.
    pub fn withdraw(ids: &[String]) {
        if !bundled() || ids.is_empty() {
            return;
        }
        let ids: Vec<Retained<NSString>> = ids.iter().map(|i| NSString::from_str(i)).collect();
        UNUserNotificationCenter::currentNotificationCenter()
            .removeDeliveredNotificationsWithIdentifiers(&NSArray::from_retained_slice(&ids));
    }

    /// Dock badge; empty at zero. Main thread only (Slint callbacks and
    /// timers run there); a no-op elsewhere.
    pub fn set_badge(count: usize) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let label = (count > 0).then(|| NSString::from_str(&count.to_string()));
        NSApplication::sharedApplication(mtm)
            .dockTile()
            .setBadgeLabel(label.as_deref());
    }

    /// Whether this is the active application (its window can take keys).
    pub fn is_active() -> bool {
        MainThreadMarker::new().is_none_or(|mtm| NSApplication::sharedApplication(mtm).isActive())
    }

    /// Asks macOS to bring the app to the front (cooperative, may be
    /// declined).
    pub fn activate() {
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm).activate();
        }
    }
}

// With the fallback on, notify() would try a real notification server, so
// there is nothing safe to call off macOS.
#[cfg(all(test, any(target_os = "macos", not(feature = "fallback"))))]
mod tests {
    use super::*;

    /// `cargo test` binaries aren't app bundles (and tests don't run on the
    /// main thread), so every call must be a safe no-op.
    #[test]
    fn calls_without_a_bundle_are_no_ops() {
        install("test", |_| {});
        notify("id", "thread", "title", "", "body");
        withdraw(&["id".to_owned()]);
        set_badge(3);
        activate();
        assert!(is_active());
        #[cfg(target_os = "macos")]
        assert!(!available());
    }
}
