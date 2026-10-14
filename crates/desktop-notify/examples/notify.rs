fn main() {
    // Run native operations on a worker: fallback posting can block. An app's
    // event loop should receive results through a channel or invoke_from_event_loop.
    std::thread::spawn(|| {
        if let Err(error) = desktop_notify::install("slint-kit example", |id| {
            println!("clicked {id}");
        }) {
            eprintln!("{error}");
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        desktop_notify::request_authorization(move |result| {
            println!("authorization: {result:?}");
            let _ = tx.send(());
        });
        // This command-line example has no UI thread. Do not wait like this in
        // a GUI callback. Unbundled macOS executables report Unsupported.
        let _ = rx.recv_timeout(std::time::Duration::from_secs(60));
        let (tx, rx) = std::sync::mpsc::channel();
        desktop_notify::notify_with_result(
            "example",
            "examples",
            "Slint kit",
            "",
            "Hello",
            move |result| {
                println!("post accepted: {result:?}");
                let _ = tx.send(());
            },
        );
        let _ = rx.recv_timeout(std::time::Duration::from_secs(60));
    })
    .join()
    .expect("notification worker");
}
