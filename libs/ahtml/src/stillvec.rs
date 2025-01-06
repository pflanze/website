//! A vector-like region allocator building block that guarantees that
//! its slots never move, and are available unchanged until
//! exclusive_clear is called, and hence allows shared references to
//! existing slots while allowing to push new items at the same
//! time. Uses internal mutability.

use std::cell::UnsafeCell;

use crate::more_vec::MoreVec;

pub struct StillVec<T>(UnsafeCell<Vec<T>>);

impl<T> StillVec<T> {
    pub fn with_capacity(cap: usize) -> Self {
        Self(UnsafeCell::new(Vec::with_capacity(cap)))
    }

    // This is used to get the new id value for allocations.
    pub fn len(&self) -> usize {
        let p = self.0.get();
        // Safe because StillVec is not Sync, hence the vector cannot
        // be mutated from under us.
        unsafe {&*p}.len()
    }

    pub fn capacity(&self) -> usize {
        let p = self.0.get();
        // Safe because StillVec since there is no API for mutating
        // the capacity.
        unsafe {&*p}.capacity()
    }

    pub fn push_within_capacity_(&self, value: T) -> Result<(), T> {
        let p = self.0.get();
        // Safe because pushing within capacity will not cause
        // reallocations, hence will not invalidate other references,
        // and StillVec is not Sync.
        unsafe {&mut *p}.push_within_capacity_(value)
    }

    pub fn get(&self, i: usize) -> Option<&T> {
        let p = self.0.get();
        // Safe because we never give mutable access to `Node`s,
        // and any stored `Node` remains in the region for the
        // duration of it's lifetime, and, we pre-allocate all
        // storage via Vec::with_capacity, so the storage will
        // never be moved.
        unsafe {&*p}.get(i)
    }

    // This must take &mut self, to ensure no references from `get`
    // still exist!
    pub fn exclusive_clear(&mut self) {
        self.0.get_mut().clear()
    }
}

