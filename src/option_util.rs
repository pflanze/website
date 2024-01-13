
// Haven't i done this already? REALLY need 1 single place for everything.
pub trait TryMap<T> {
    fn try_map<F, U, E>(self, f: F) -> Result<Option<U>, E>
    where F: FnOnce(T) -> Result<U, E>;
}

impl<T> TryMap<T> for Option<T> {
    fn try_map<F, U, E>(self, f: F) -> Result<Option<U>, E>
    where F: FnOnce(T) -> Result<U, E>
    {
        match self {
            Some(v) => Ok(Some(f(v)?)),
            None => Ok(None)
        }
    }
}

