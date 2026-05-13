use notify::{EventKind, RecursiveMode, Result, Watcher};
use std::path::Path;
use tokio::runtime::Handle;
use zbus::object_server::SignalEmitter;
use zbus::{connection::Builder, interface};

struct WatcherSignal;

#[interface(name = "org.kde.FileWatcher")]
impl WatcherSignal {
    #[zbus(signal)]
    async fn file_changed(ctx: &SignalEmitter<'_>, contents: &str) -> zbus::Result<()>;
}

#[tokio::main]
async fn main() -> Result<()> {
    // session bus, not system bus
    let conn = Builder::session()
        .expect("builder failed")
        .name("org.kde.FileWatcher")
        .expect("builder failed")
        .serve_at("/Object", WatcherSignal)
        .expect("builder failed")
        .build()
        .await
        .unwrap();

    let ctx: zbus::object_server::InterfaceRef<WatcherSignal> =
        conn.object_server().interface("/Object").await.unwrap();

    let handle = Handle::current();

    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event>| {
        if let Ok(event) = res {
            handle.block_on(async {
                if let Some(path) = event.paths.first()
                    && matches!(event.kind, EventKind::Modify(_))
                {
                    let message = tokio::fs::read_to_string(path)
                        .await
                        .expect("failed to read file lmao");
                    ctx.file_changed(&message).await.expect("lol");
                }
            })
        }
    })?;

    watcher.watch(Path::new("/dev/shm/lollipop.shm"), RecursiveMode::Recursive)?;

    std::future::pending::<()>().await; // aka wait forever
    Ok(())
}
