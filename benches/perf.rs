use std::hint::black_box;

use static_reload::{Resource, ResourceCell, kind::ReloadableRef};

pub(crate) struct Noop;

impl Resource for Noop {
    type Definition = ();
    type Error = std::convert::Infallible;

    fn load(_: &Self::Definition) -> impl Future<Output = Result<Self, Self::Error>> {
        std::future::ready(Ok(Self))
    }
}

static RESOURCE: ResourceCell<Noop> = ResourceCell::new();
static RELOADED_RESOURCE: ResourceCell<Noop> = ResourceCell::new();

// TODO: support of variable thread count benchmarks requires native async support in divan
// (or a switch of harness)

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // TODO: currently THREADS are mostly meaningless
    // since we init outside
    RESOURCE.init(()).await;

    RELOADED_RESOURCE.init(()).await;
    RELOADED_RESOURCE.reload().await;

    divan::main();
}

#[divan::bench]
fn load_resource<'a>() -> ReloadableRef<'a, Noop> {
    black_box(&RESOURCE).require()
}

#[divan::bench]
fn load_reloaded_resource<'a>() -> ReloadableRef<'a, Noop> {
    black_box(&RELOADED_RESOURCE).require()
}
