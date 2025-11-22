use std::sync::Arc;

use log::trace;
use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard};
use type_map::TypeMap;

#[derive(Debug, Clone)]
pub struct Resources(pub Arc<RwLock<TypeMap>>);

impl Resources {
    /// Creates a new resource map.
    pub fn new(map: TypeMap) -> Self {
        Self(Arc::new(RwLock::new(map)))
    }

    /// Gets a ref to a resource from the resource map. Panics if the resource is not present.
    pub fn get<T: 'static>(&self) -> MappedRwLockReadGuard<'_, T> {
        trace!("getting ref to resource: {:?}", std::any::type_name::<T>());
        RwLockReadGuard::map(self.0.read(), |x| x.get::<T>().unwrap())
    }

    /// Sets a resource in the resource map.
    pub fn insert<T: 'static>(&self, value: T) {
        self.0.write().insert(value);
    }
}
