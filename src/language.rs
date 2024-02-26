use crate::str_util::str_take;

pub trait Language: Clone + Copy + PartialEq + Eq + Default + Send + Sync {
    type MemberIter: Iterator<Item = Self>;

    // XX use some parse trait instead ?

    fn maybe_from(s: &str) -> Option<Self> where Self: Sized;

    fn maybe_from_start(s: &str) -> Option<Self> {
        let (start, ok) = str_take(s, 2);
        if ! ok { return None }
        Self::maybe_from(start)
    }

    /// 2-letter lower-case language code.
    fn as_str(self) -> &'static str;

    /// In the order in which they should be listed in the language
    /// switcher.
    fn members() -> Self::MemberIter;

    // XX generate from members?
    fn strs() -> &'static [&'static str];
}

