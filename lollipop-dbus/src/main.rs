use anyhow::Result;
use notify::{EventKind, RecursiveMode, Watcher};
use std::path::Path;
use tokio::runtime::Handle;
use zbus::object_server::SignalEmitter;
use zbus::{connection::Builder, interface};

struct WatcherSignal;

#[interface(name = "xyz.lavafroth.Lollipop")]
impl WatcherSignal {
    #[zbus(signal)]
    async fn file_changed(ctx: &SignalEmitter<'_>, contents: &str) -> zbus::Result<()>;
}

#[tokio::main]
async fn main() -> Result<()> {
    // session bus, not system bus
    let conn = Builder::session()?
        .name("xyz.lavafroth.Lollipop")?
        .serve_at("/Object", WatcherSignal)?
        .build()
        .await?;

    let ctx = conn.object_server().interface("/Object").await?;
    let handle = Handle::current();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            handle.block_on(async {
                if let Some(path) = event.paths.first()
                    && matches!(event.kind, EventKind::Modify(_))
                {
                    let message = tokio::fs::read_to_string(path)
                        .await
                        .expect("failed to read shared memory");
                    ctx.file_changed(&message)
                        .await
                        .expect("failed to notify changes via dbus");
                }
            })
        }
    })?;

    watcher.watch(
        Path::new("/dev/shm/lollipop.shm"),
        RecursiveMode::NonRecursive,
    )?;

    std::future::pending().await // aka wait forever
}
