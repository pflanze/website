use kstring::KString;
use pulldown_cmark::CowStr;

// Can't impl `ahtml::myfrom::MyFrom`, and making an own trait again
// does not help with the usage of `ahtml`. Will have to explicitly
// convert via a function instead.

pub trait MyFrom2<T> {
    fn myfrom(s: T) -> Self;    
}

impl<'t> MyFrom2<&CowStr<'t>> for KString {
    fn myfrom(s: &CowStr<'t>) -> Self {
        KString::from_ref(s.as_ref())
    }
}

impl<'t> MyFrom2<CowStr<'t>> for KString {
    fn myfrom(s: CowStr<'t>) -> Self {
        KString::from_ref(s.as_ref())
    }
}

// impl<'t> MyFromInclStr<&'t CowStr<'t>> for &'t str {
//     fn myfrom(s: &'t CowStr<'t>) -> Self {
//         s
//     }
// }

impl<'t> MyFrom2<&'t CowStr<'t>> for &'t str {
    fn myfrom(s: &'t CowStr<'t>) -> Self {
        s
    }
}

pub fn kstring_myfrom2<From>(from: From) -> KString
    where KString: MyFrom2<From>
{
    KString::myfrom(from)
}

