use crate::str_util::str_take;

pub trait Language: Clone + Copy + PartialEq + Eq + Default + Send + Sync {
    // XX use some parse trait instead ?

    fn maybe_from(s: &str) -> Option<Self> where Self: Sized;

    fn maybe_from_start(s: &str) -> Option<Self> {
        let (start, ok) = str_take(s, 2);
        if ! ok { return None }
        Self::maybe_from(start)
    }

    fn strs() -> &'static [&'static str];
}

