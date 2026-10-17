# desktop-clipboard

Read clipboard attachments as file lists or encoded images, then convert images
to PNG with bounded decoding on a worker thread. Ordinary text can remain in
the native text input's paste flow. This crate does not depend on Slint.

```toml
[dependencies]
desktop-clipboard = "0.1.0"
```

```rust,no_run
use desktop_clipboard::{ClipboardAttachment, clipboard_attachment, to_png};

match clipboard_attachment()? {
    Some(ClipboardAttachment::Files(paths)) => println!("{paths:?}"),
    Some(ClipboardAttachment::Image(encoded)) => {
        std::thread::spawn(move || {
            let png = to_png(encoded)?;
            // Send the bytes back to the UI event loop.
            Ok::<_, desktop_clipboard::Error>(png)
        });
    }
    None => { /* Let the native input handle text paste. */ }
}
# Ok::<(), desktop_clipboard::Error>(())
```

File lists take priority over alternate image representations. Recognized but
unreadable content returns a typed `Error`; library failures retain their
underlying `Error::source()`.

On macOS, image acquisition snapshots encoded PNG, TIFF, or JPEG data. Elsewhere,
images return `None` unless the `decoding-fallback` feature is enabled. That
feature uses arboard's decoded RGBA, so decoding occurs before size checks.

`to_png()` limits encoded input to 32 MiB, each dimension to 8192 pixels, total
pixels to 16 Mi, and decoder allocations to 128 MiB. Run conversion off the UI
thread. File lists remain available without the image fallback.

From a checkout, `cargo run -p desktop-clipboard --example paste` reads the real
clipboard. Add `--features decoding-fallback` for images on Linux or Windows.

Part of [slint-kit](https://github.com/tilladam/slint-kit).
[API documentation](https://docs.rs/desktop-clipboard). MIT licensed.
