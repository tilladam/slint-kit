# desktop-notify

Desktop notifications with observable authorization and posting results. macOS
supports click routing by notification id, withdrawal, Dock badges, and app
activation. The optional fallback uses notify-rust on Linux and Windows.
This crate does not depend on Slint or an async runtime.

```toml
[dependencies]
desktop-notify = "0.1.0"
```

```rust,no_run
desktop_notify::install("My app", |id| {
    // Send the id to the UI event loop.
    println!("open {id}");
})?;
desktop_notify::request_authorization(|result| println!("permission: {result:?}"));
// Post after authorization completes in an application.
desktop_notify::notify_with_result("message:1", "conversation:1", "New message", "", "Hello", |result| {
    println!("request accepted: {result:?}");
});
# Ok::<(), desktop_notify::Error>(())
```

Call `install()` once before the event loop; repeated installation returns
`AlreadyInstalled`. Request permission explicitly, and query it with
`authorization_status()`. `capabilities()` reports platform support separately
from permission or service availability. Posting success means the backend
accepted the request; it does not guarantee delivery.

Callbacks may run synchronously or on native background threads. Marshal UI
work onto the event loop. On macOS, notification operations require an app
bundle with a bundle identifier; plain `cargo run` binaries return
`Unsupported`. Badge and activation operations belong on the main thread.

Enable `fallback` on Linux or Windows to show notifications. Fallback posting
can block and belongs on a worker thread. It does not support click routing,
withdrawal, badges, or replacement by id. Authorization status is `Unknown`,
and permission requests return `Unsupported`.

`notify()` is a fire-and-log convenience wrapper. From a checkout,
`cargo run -p desktop-notify --example notify --features fallback` demonstrates
callbacks on a supported desktop; unbundled macOS binaries report unsupported
operations.

Part of [slint-kit](https://github.com/tilladam/slint-kit).
[API documentation](https://docs.rs/desktop-notify). MIT licensed.
