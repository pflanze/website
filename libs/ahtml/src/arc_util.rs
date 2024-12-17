use std::sync::Arc;

/// Make it possible to pass &x or x to a function, where x is
/// `Arc<T>`, and have the function do the clone operation when
/// needed.
pub trait IntoArc<T> {
    fn into_arc(self) -> Arc<T>;
}

impl<T> IntoArc<T> for T {
    fn into_arc(self) -> Arc<T> {
        Arc::new(self)
    }
}
impl<T> IntoArc<T> for Arc<T> {
    fn into_arc(self) -> Arc<T> {
        self
    }
}
impl<T> IntoArc<T> for &Arc<T> {
    fn into_arc(self) -> Arc<T> {
        Arc::clone(self)
    }
}


