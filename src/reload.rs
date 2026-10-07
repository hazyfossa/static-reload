use std::sync::Arc;

use hazarc::{AtomicArc, Cache, atomic::CachedOrReloaded};

use crate::{Resource, ResourceCell, kind, private::Sealed};

pub struct ReloadableMeta<T: Resource> {
    pub(crate) definition: T::Definition,
    pub(crate) cached_ptr: hazarc::Cache<AtomicArc<T>>,
}

// A reference derived from a pointer
// that may or may not have been reloaded
pub type Ref<'a, T> = CachedOrReloaded<'a, Arc<T>>;

impl Sealed for kind::Reloadable {}
impl kind::T for kind::Reloadable {
    type Body<T: Resource> = ReloadableMeta<T>;
    type ResourceRef<'a, T: 'a> = Ref<'a, T>;

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
