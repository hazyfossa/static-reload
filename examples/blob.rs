use std::{fs, io, path::PathBuf};

use static_resource::{Resource, ResourceCell};
use tokio::signal::unix::SignalKind;

struct FileData(Vec<u8>);

impl Resource for FileData {
    type Definition = PathBuf;
    type Error = io::Error;

    async fn load(path: &Self::Definition) -> Result<Self, Self::Error> {
        let buf = fs::read(path)?;
        Ok(Self(buf))
    }
}

static BLOB: ResourceCell<FileData> = ResourceCell::new();

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // You will probably get the definition from cli args or config
    let path = "./large_file".into();

    BLOB.init(path).await.unwrap();

    any_function().await
}

async fn any_function() {
    // Reading a resource is very fast (and doesn't require .await!)
    let blob = BLOB.require();
    println!("The length of blob is: {} bytes", blob.0.len());

    // You can trigger a reload from anywhere
    BLOB.reload().await.unwrap();

    reload_on_sighup();
}

// A common pattern for resources
// Note how BLOB easily crosses 'static task boundaries
fn reload_on_sighup() {
    tokio::spawn(async move {
        let mut sighup = tokio::signal::unix::signal(SignalKind::hangup())
            .expect("Cannot setup signal handling");

        loop {
            sighup.recv().await;

            // You may want to emit a warning here instead of .expect'ing
            BLOB.reload().await.expect("Failed to update blob");
        }
    });
}
