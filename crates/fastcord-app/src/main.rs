//! FastCord binary entry: builds the tokio runtime (#13) and starts the app.
//!
//! The UI thread is the winit main thread and is never a tokio worker; every
//! network/disk task runs on this runtime and reports back through channels.
//! Nothing calls `Handle::current()`; the handle is injected into services.

use tokio::runtime::Builder;

fn main() {
    let runtime = Builder::new_multi_thread()
        .worker_threads(3)
        .enable_all()
        .build()
        .expect("tokio runtime builds");
    runtime.block_on(async {
        tracing::debug!(version = env!("CARGO_PKG_VERSION"), "fastcord starting");
    });
}
