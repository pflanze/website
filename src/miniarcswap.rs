use std::sync::{Mutex, Arc};

// Use arc-swap instead? But see
// https://docs.rs/arc-swap/latest/arc_swap/docs/limitations/index.html

/// Stores an Arc<T>, hands it out to any number of readers for any
/// length of time, and allows the Arc<T> to be replaced at any
/// time. Reminiscent of the `arc_swap` crate, or maybe of RCU
/// (read-copy-update) although MiniArcSwap is comparatively slow,
/// both for the Arc usage and for the Mutex around it (which might be
/// omitted / replaced with atomics, todo).
pub struct MiniArcSwap<T> {
    payload: Mutex<Arc<T>>
}

impl<T> MiniArcSwap<T> {
    /// Wrap the payload with the MiniArcSwap.
    pub fn new(payload: Arc<T>) -> MiniArcSwap<T> {
        MiniArcSwap { payload: Mutex::new(payload) }
    }
    /// Get the payload. Use it however long you want. This call
    /// finishes almost immediately.
    pub fn get(&self) -> Arc<T> {
        Arc::clone(&(*self.payload.lock().expect("never poisoned")))
    }
    /// Set the payload. This call finishes almost immediately and
    /// does not block any readers. From this instant on, `get`
    /// returns the new payload.
    pub fn set(&self, val: Arc<T>) {
        *self.payload.lock().expect("never poisoned") = val;
    }
}

// Deref would require a separate guard holding the Arc. Arc
// wrapper. Well, Arc itself does Deref already. So, forget about
// it. Just *have* to have setter and getter methods above.

// impl<T> Deref for MiniArcSwap<T> {
//     type Target = Arc<T>;

//     fn deref(&self) -> &Self::Target {
//         Arc::clone(&(*self.payload.lock().expect("never poisoned")))
//     }
// }

// impl<T> DerefMut for MiniArcSwap<T> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         self.blogcache.lock().expect("never poisoned")
//         Arc::clone(&self.blogcache.lock().expect("never poisoned"))
//     }
// }
