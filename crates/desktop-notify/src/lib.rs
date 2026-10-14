//! Desktop notifications with click-to-open, the Dock badge, and bringing the
//! app to the front.
//!
//! - **macOS**: UserNotifications (`UNUserNotificationCenter`). A click,
//!   including one that launches the app, calls the `install` callback with
//!   the notification's id. UserNotifications aborts the process when there is
//!   no bundle identity, so every call is guarded: a plain `cargo run` binary
//!   (not inside a `.app`) reports unsupported notification operations.
//! - **Elsewhere**: no-ops, unless the `fallback` feature shows notifications
//!   through notify-rust (show-only: no click routing, withdraw or badge).
//!
//! Ids are the caller's: encode whatever a click needs to find its target
//! again (yapper uses `"{account}:{message_id}"`). Diagnostics go to the `log`
//! facade for the convenience notify wrapper.
//!
//! Extracted from yapper (`crates/yapper-ui/src/platform.rs`).
//!
//! Callbacks may complete synchronously for unsupported operations or on native
//! background threads. Marshal UI updates onto the event loop. `install` no
//! longer requests permission implicitly; call `request_authorization` separately.
//! The fallback's posting call can block, so use a worker thread there.
//!
//! ```no_run
//! desktop_notify::install("My app", |id| {
//!     // Send the id to the UI event loop; do not access widgets here.
//!     println!("open {id}");
//! })?;
//! desktop_notify::request_authorization(|result| println!("permission: {result:?}"));
//! desktop_notify::authorization_status(|result| println!("status: {result:?}"));
//! desktop_notify::notify_with_result("message:1", "conversation:1", "New message", "", "Hello", |result| {
//!     println!("request accepted: {result:?}");
//! });
//! # Ok::<(), desktop_notify::Error>(())
//! ```

/// Platform support, independent of authorization or service availability.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub notifications: bool,
    pub clicks: bool,
    pub withdraw: bool,
    pub badge: bool,
    pub activation: bool,
}

/// Authorization status. `Unknown` means this platform cannot query it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Authorization {
    NotDetermined,
    Denied,
    Authorized,
    Provisional,
    Ephemeral,
    Unknown,
}

/// Installation, authorization, or posting failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Unsupported,
    AlreadyInstalled,
    PermissionDenied,
    Native {
        operation: &'static str,
        domain: String,
        code: isize,
        message: String,
    },
    Backend(Box<dyn std::error::Error + Send + Sync>),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => f.write_str("Notifications are unsupported in this configuration"),
            Self::AlreadyInstalled => f.write_str("Notification handler already installed"),
            Self::PermissionDenied => f.write_str("Notification permission denied"),
            Self::Native {
                operation,
                domain,
                code,
                message,
            } => write!(f, "Notifications {operation}: {domain} ({code}): {message}"),
            Self::Backend(source) => write!(f, "Notifications backend: {source}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Backend(source) = self {
            Some(source.as_ref())
        } else {
            None
        }
    }
}

fn install_once<T>(slot: &std::sync::OnceLock<T>, value: T) -> Result<(), Error> {
    slot.set(value).map_err(|_| Error::AlreadyInstalled)
}

#[cfg(any(target_os = "macos", test))]
fn route_click(handler: &std::sync::OnceLock<Box<dyn Fn(String) + Send + Sync>>, id: String) {
    if let Some(handler) = handler.get() {
        handler(id);
    }
}

// Native blocks are Fn, while a caller should be able to pass FnOnce. Release
// the lock before invoking user code, including when a backend calls twice.
#[cfg(any(target_os = "macos", test))]
fn once_callback<T>(done: impl FnOnce(T) + Send + 'static) -> impl Fn(T) + Send + Sync + 'static {
    let done = std::sync::Mutex::new(Some(done));
    move |result| {
        let callback = done.lock().expect("callback lock").take();
        if let Some(callback) = callback {
            callback(result);
        }
    }
}

#[cfg(any(all(not(target_os = "macos"), feature = "fallback"), test))]
fn fallback_body(subtitle: &str, body: &str) -> String {
    if subtitle.is_empty() {
        body.to_owned()
    } else {
        format!("{subtitle}\n{body}")
    }
}

/// Fire-and-log convenience wrapper. Use [`notify_with_result`] to observe
/// whether the backend accepted the request; success does not guarantee delivery.
pub fn notify(id: &str, thread: &str, title: &str, subtitle: &str, body: &str) {
    notify_with_result(id, thread, title, subtitle, body, |result| {
        if let Err(error) = result {
            log::warn!("{error}");
        }
    });
}

#[cfg(target_os = "macos")]
pub use mac::*;

#[cfg(not(target_os = "macos"))]
pub use other::*;

#[cfg(not(target_os = "macos"))]
mod other {
    use super::{Authorization, Capabilities, Error, install_once};
    /// Installs the app name once. Click routing is unsupported off macOS.
    pub fn install(
        app_name: &str,
        _on_click: impl Fn(String) + Send + Sync + 'static,
    ) -> Result<(), Error> {
        if !available() {
            return Err(Error::Unsupported);
        }
        install_once(&APP_NAME, app_name.to_owned())
    }
    static APP_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();

    /// Whether the notification backend is enabled, not whether permission is granted.
    pub fn available() -> bool {
        capabilities().notifications
    }
    pub fn capabilities() -> Capabilities {
        Capabilities {
            notifications: cfg!(feature = "fallback"),
            ..Default::default()
        }
    }

    /// Completes synchronously with Unknown when fallback is enabled.
    pub fn authorization_status(done: impl FnOnce(Result<Authorization, Error>) + Send + 'static) {
        done(if available() {
            Ok(Authorization::Unknown)
        } else {
            Err(Error::Unsupported)
        });
    }
    /// Permission requests are not exposed by the fallback backend.
    pub fn request_authorization(done: impl FnOnce(Result<Authorization, Error>) + Send + 'static) {
        done(Err(Error::Unsupported));
    }

    /// The fallback may block while contacting the OS service: call on a worker
    /// thread. Completion runs synchronously on the calling thread. Id/thread
    /// replacement, click routing, withdrawal, and badges are unsupported.
    pub fn notify_with_result(
        _id: &str,
        _thread: &str,
        title: &str,
        subtitle: &str,
        body: &str,
        done: impl FnOnce(Result<(), Error>) + Send + 'static,
    ) {
        #[cfg(feature = "fallback")]
        {
            let mut notification = notify_rust::Notification::new();
            notification
                .summary(title)
                .body(&super::fallback_body(subtitle, body));
            if let Some(name) = APP_NAME.get() {
                notification.appname(name);
            }
            done(
                notification
                    .show()
                    .map(|_| ())
                    .map_err(|source| Error::Backend(Box::new(source))),
            );
        }
        #[cfg(not(feature = "fallback"))]
        {
            let _ = (title, subtitle, body);
            done(Err(Error::Unsupported));
        }
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
    use super::{Authorization, Capabilities, Error, install_once, once_callback, route_click};
    use std::sync::OnceLock;

    use block2::{DynBlock, RcBlock};
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, NSObject, ProtocolObject};
    use objc2::{AnyThread, MainThreadMarker, define_class, msg_send};
    use objc2_app_kit::NSApplication;
    use objc2_foundation::{NSArray, NSBundle, NSError, NSObjectProtocol, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent,
        UNNotification, UNNotificationPresentationOptions, UNNotificationRequest,
        UNNotificationResponse, UNNotificationSettings, UNNotificationSound,
        UNUserNotificationCenter, UNUserNotificationCenterDelegate,
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
                route_click(&ON_CLICK, id);
            }
        }
    );

    /// Installs the click handler once. Call before the event loop runs so a
    /// launch from a notification can be routed. The callback runs on an arbitrary
    /// thread. Request permission separately with request_authorization.
    /// `app_name` is ignored: the macOS app bundle supplies its name.
    pub fn install(
        _app_name: &str,
        on_click: impl Fn(String) + Send + Sync + 'static,
    ) -> Result<(), Error> {
        if !bundled() {
            return Err(Error::Unsupported);
        }
        install_once(&ON_CLICK, Box::new(on_click))?;
        let center = UNUserNotificationCenter::currentNotificationCenter();
        // SAFETY: Delegate is an NSObject subclass with NSObject's init method.
        let delegate: Retained<Delegate> = unsafe { msg_send![Delegate::alloc(), init] };
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // The delegate property is weak; keep the single delegate for the process.
        std::mem::forget(delegate);
        Ok(())
    }

    /// Bundle eligibility only; use authorization_status for permission.
    pub fn available() -> bool {
        bundled()
    }
    pub fn capabilities() -> Capabilities {
        let enabled = bundled();
        Capabilities {
            notifications: enabled,
            clicks: enabled,
            withdraw: enabled,
            badge: true,
            activation: true,
        }
    }

    fn native_error(operation: &'static str, error: &NSError) -> Error {
        Error::Native {
            operation,
            domain: error.domain().to_string(),
            code: error.code(),
            message: error.localizedDescription().to_string(),
        }
    }
    fn status(status: UNAuthorizationStatus) -> Authorization {
        match status {
            UNAuthorizationStatus::NotDetermined => Authorization::NotDetermined,
            UNAuthorizationStatus::Denied => Authorization::Denied,
            UNAuthorizationStatus::Authorized => Authorization::Authorized,
            UNAuthorizationStatus::Provisional => Authorization::Provisional,
            UNAuthorizationStatus::Ephemeral => Authorization::Ephemeral,
            _ => Authorization::Unknown,
        }
    }

    /// Completes on an arbitrary thread, or synchronously if unbundled.
    pub fn authorization_status(done: impl FnOnce(Result<Authorization, Error>) + Send + 'static) {
        if !bundled() {
            done(Err(Error::Unsupported));
            return;
        }
        let done = once_callback(done);
        let block = RcBlock::new(move |settings: std::ptr::NonNull<UNNotificationSettings>| {
            // SAFETY: UserNotifications supplies valid settings for this callback.
            let settings = unsafe { settings.as_ref() };
            done(Ok(status(settings.authorizationStatus())));
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .getNotificationSettingsWithCompletionHandler(&block);
    }

    /// Requests alert, sound, and badge permission. Completes on an arbitrary
    /// thread; never wait for it on the UI thread. Denial is PermissionDenied.
    pub fn request_authorization(done: impl FnOnce(Result<Authorization, Error>) + Send + 'static) {
        if !bundled() {
            done(Err(Error::Unsupported));
            return;
        }
        let done = once_callback(done);
        let block = RcBlock::new(move |granted: Bool, error: *mut NSError| {
            // SAFETY: UserNotifications lends a nullable NSError during completion.
            let result = match unsafe { error.as_ref() } {
                Some(error) => Err(native_error("requesting permission", error)),
                None if granted.as_bool() => Ok(Authorization::Authorized),
                None => Err(Error::PermissionDenied),
            };
            done(result);
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert
                    | UNAuthorizationOptions::Sound
                    | UNAuthorizationOptions::Badge,
                &block,
            );
    }

    /// Posts asynchronously; the callback reports request acceptance, not
    /// delivery, on an arbitrary thread. The same id replaces an earlier request.
    pub fn notify_with_result(
        id: &str,
        thread: &str,
        title: &str,
        subtitle: &str,
        body: &str,
        done: impl FnOnce(Result<(), Error>) + Send + 'static,
    ) {
        if !bundled() {
            done(Err(Error::Unsupported));
            return;
        }
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setSubtitle(&NSString::from_str(subtitle));
        content.setBody(&NSString::from_str(body));
        content.setThreadIdentifier(&NSString::from_str(thread));
        content.setSound(Some(&UNNotificationSound::defaultSound()));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(id),
            &content,
            None,
        );
        let done = once_callback(done);
        let block = RcBlock::new(move |error: *mut NSError| {
            // SAFETY: UserNotifications lends a nullable NSError during completion.
            done(match unsafe { error.as_ref() } {
                Some(error) => Err(native_error("posting", error)),
                None => Ok(()),
            });
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .addNotificationRequest_withCompletionHandler(&request, Some(&block));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_install_preserves_the_original_handler() {
        let slot: std::sync::OnceLock<Box<dyn Fn(String) + Send + Sync>> =
            std::sync::OnceLock::new();
        let (tx, rx) = std::sync::mpsc::channel();
        install_once(
            &slot,
            Box::new(move |id| {
                tx.send(id).unwrap();
            }),
        )
        .unwrap();
        assert!(matches!(
            install_once(&slot, Box::new(|_| panic!("replaced"))),
            Err(Error::AlreadyInstalled)
        ));
        route_click(&slot, "target".into());
        assert_eq!(rx.recv().unwrap(), "target");
    }
    #[test]
    fn completion_runs_once_and_fallback_format_is_testable() {
        let (tx, rx) = std::sync::mpsc::channel();
        let done = once_callback(move |value| {
            tx.send(value).unwrap();
        });
        done(1);
        done(2);
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), [1]);
        assert_eq!(fallback_body("", "body"), "body");
        assert_eq!(fallback_body("subtitle", "body"), "subtitle\nbody");
    }
    #[cfg(any(target_os = "macos", not(feature = "fallback")))]
    #[test]
    fn unsupported_configuration_reports_errors_without_os_services() {
        assert!(matches!(install("test", |_| {}), Err(Error::Unsupported)));
        notify_with_result("id", "thread", "title", "", "body", |r| {
            assert!(matches!(r, Err(Error::Unsupported)))
        });
        authorization_status(|r| assert!(matches!(r, Err(Error::Unsupported))));
        request_authorization(|r| assert!(matches!(r, Err(Error::Unsupported))));
        withdraw(&["id".into()]);
        set_badge(3);
        activate();
        assert!(is_active());
    }
}
