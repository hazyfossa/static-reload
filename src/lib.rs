use std::{
    any::type_name,
    sync::{Arc, OnceLock},
};

pub mod kind {
    use std::sync::Arc;

    use hazarc::{AtomicArc, Cache, atomic::CachedOrReloaded};
    use private::Sealed;

    use crate::Resource;
    mod private {
        pub trait Sealed {}
    }

    pub trait T: Sealed {
        type Body<T: Resource>;
        type ResourceRef<'a, T: 'a>;

        fn define<T: Resource>(definition: T::Definition, instance: T) -> Self::Body<T>;
        fn load<'a, T: Resource>(ptr: &Self::Body<T>) -> Self::ResourceRef<'_, T>;
    }

    pub struct Static;
    impl Sealed for Static {}
    impl T for Static {
        type Body<T: Resource> = T;
        type ResourceRef<'a, T: 'a> = &'a T;

        fn define<T: Resource>(_: T::Definition, instance: T) -> Self::Body<T> {
            instance
        }

        fn load<'a, T: Resource>(ptr: &Self::Body<T>) -> Self::ResourceRef<'_, T> {
            &ptr
        }
    }

    // Store resource definition inline to allow for updates
    pub struct ReloadableMeta<T: Resource> {
        pub(crate) definition: T::Definition,
        pub(crate) cached_ptr: hazarc::Cache<AtomicArc<T>>,
    }

    pub type ReloadableRef<'a, T> = CachedOrReloaded<'a, Arc<T>>;

    pub struct Reloadable;
    impl Sealed for Reloadable {}
    impl T for Reloadable {
        type Body<T: Resource> = ReloadableMeta<T>;
        type ResourceRef<'a, T: 'a> = ReloadableRef<'a, T>;

        fn define<T: Resource>(definition: T::Definition, instance: T) -> Self::Body<T> {
            let ptr = Arc::new(instance);
            let hazard_ptr = AtomicArc::new(ptr);
            let cached_ptr = Cache::new(hazard_ptr);

            ReloadableMeta {
                definition,
                cached_ptr,
            }
        }

        fn load<'a, T: Resource>(ptr: &Self::Body<T>) -> Self::ResourceRef<'_, T> {
            ptr.cached_ptr.load_shared()
        }
    }
}

// TODO: if we abandon Cache, we can return fully owned ArcBorrows from read,
// turning resources from `static` to `const`
//
// while for our very-infrequent-update case the benefits of Cache (probably) outweight
// drawbacks, a proper benchmark would be nice

#[allow(async_fn_in_trait)]
// TODO: allow unsized resources which manage their own Arc layout
pub trait Resource: Sized {
    type Definition;
    type Error;

    fn name() -> &'static str {
        type_name::<Self>()
    }

    async fn load(definition: &Self::Definition) -> Result<Self, Self::Error>;
}

pub struct ResourceCell<T: Resource + 'static, Kind: kind::T = kind::Reloadable> {
    cell: OnceLock<Kind::Body<T>>,
}

impl<T: Resource, Kind: kind::T> ResourceCell<T, Kind> {
    pub const fn new() -> Self {
        Self {
            cell: OnceLock::new(),
        }
    }

    /// Init can only be called once per ResourceCell
    /// It is recommended to call it from `main`
    pub async fn init(&self, definition: T::Definition) -> Result<(), T::Error> {
        let instance = T::load(&definition).await?;
        let body = Kind::define(definition, instance);

        let ret = self.cell.set(body);
        if ret.is_err() {
            panic!("'{}' is initialized twice", T::name())
        }

        Ok(())
    }

    fn expect_init(&self) -> &Kind::Body<T> {
        self.cell
            .get()
            .expect(&format!("'{}' not initialized", T::name()))
    }

    /// This function is very cheap to call
    ///
    /// For initial data (before a reload), performance should be
    /// comparable to a 'static pointer dereference under black_box
    ///
    /// For hot-reloaded data, performance is comparable to
    /// loading from arc-swap (still very fast)
    pub fn require(&self) -> Kind::ResourceRef<'_, T> {
        let this = self.expect_init();
        Kind::load(&this)
    }
}

impl<T: Resource> ResourceCell<T, kind::Reloadable> {
    pub fn manual_update(&self, new: T) {
        let this = self.expect_init();

        let new_ptr = Arc::new(new);
        this.cached_ptr.inner().store(new_ptr);
    }

    pub async fn reload(&self) -> Result<(), T::Error> {
        let this = self.expect_init();

        let new_instance = T::load(&this.definition).await?;
        let new_ptr = Arc::new(new_instance);

        this.cached_ptr.inner().store(new_ptr);

        Ok(())
    }
}

#[cfg(feature = "bundle")]
#[doc(hidden)]
pub use paste::paste as __paste;

// TODO: proper error here (anyhow/eyre?)
#[cfg(feature = "bundle")]
#[macro_export]
macro_rules! resources {
    ($vis:vis $name:ident {
        $($resource:ident: $type:ty),* $(,)?
    }) => {
        $vis mod $name { $crate::__paste! {
            use super::*;
            use $crate::{Resource, ResourceCell};

            $(pub static [<$resource:upper>]: ResourceCell<$type> = ResourceCell::new();)*

            pub async fn init($([<$resource:lower>]: <$type as Resource>::Definition),*) -> Result<(), String> {
                let ret;
                $crate::resources!(@parallel "Initializing" ret => {
                    $($resource.init([<$resource:lower>]))* }
                );
                ret
            }


            pub async fn reload_all() -> Result<(), String> {
                let ret;
                $crate::resources!(@parallel "Reloading" ret => { $($resource.reload())* });
                ret
            }
        }}
    };

    (@parallel $action:literal $ret:ident => { $( $resource:ident . $fn:tt($($arg:tt)?) )* }) => { $crate::__paste! {
        let mut tasks = tokio::task::JoinSet::new();

        $(tasks.spawn(async move {
            [<$resource:upper>].$fn($($arg)?).await
            .map_err(|e| format!("{} resource {} failed: {e:?}", $action, stringify!($resource)))
        });)*

        $ret = tasks.join_all().await.into_iter().collect();
    }};
}
