use desktop_clipboard::{ClipboardAttachment, clipboard_attachment, to_png};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match clipboard_attachment()? {
        Some(ClipboardAttachment::Files(files)) => println!("files: {files:?}"),
        Some(ClipboardAttachment::Image(image)) => {
            // In an app, send this result back to the event loop instead of
            // joining the worker on the UI thread.
            let worker = std::thread::spawn(move || to_png(image));
            let png = worker.join().expect("conversion worker")?;
            println!("PNG: {} bytes", png.len());
        }
        None => println!("No attachment; let the text input handle paste"),
    }
    Ok(())
}
