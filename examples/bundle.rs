use std::{fs, io, path::PathBuf};

use static_reload::{Resource, resources};

#[allow(unused)]
struct FileData(Vec<u8>);

impl Resource for FileData {
    type Definition = PathBuf;
    type Error = io::Error;

    async fn load(path: &Self::Definition) -> Result<Self, Self::Error> {
        let buf = fs::read(path)?;
        Ok(Self(buf))
    }
}

resources!(resources {
    favicon: FileData,
    other: FileData,
});

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // bundle::init will initialize resources in paraller
    resources::init("/assets/favicon".into(), "/assets/other".into())
        .await
        .unwrap();

    // reloading is also parallelized
    resources::reload_all().await.unwrap();
}
