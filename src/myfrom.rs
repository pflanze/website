use std::borrow::Cow;

use kstring::KString;
use pulldown_cmark::CowStr;

// FUTURE: figure out how to inherit from `From` (and keep all the
// existing From definitions for KString). It doesn't work out of the
// box (various errors).
pub trait MyFrom<T> {
    fn myfrom(s: T) -> Self;    
}

// Can't do KString::from_static: no way to have a separate trait impl
// for &'static.
impl MyFrom<&str> for KString {
    fn myfrom(s: &str) -> Self {
        KString::from_ref(s)
    }
}

impl MyFrom<&&str> for KString {
    fn myfrom(s: &&str) -> Self {
        KString::from_ref(*s)
    }
}

impl MyFrom<&String> for KString {
    fn myfrom(s: &String) -> Self {
        KString::from_ref(s)
    }
}

impl MyFrom<String> for KString {
    fn myfrom(s: String) -> Self {
        KString::from_string(s)
    }
}

impl MyFrom<&KString> for KString {
    fn myfrom(s: &KString) -> Self {
        s.clone()
    }
}

impl MyFrom<KString> for KString {
    fn myfrom(s: KString) -> Self {
        s
    }
}

impl<'t> MyFrom<&CowStr<'t>> for KString {
    fn myfrom(s: &CowStr<'t>) -> Self {
        KString::from_ref(s.as_ref())
    }
}

impl<'t> MyFrom<CowStr<'t>> for KString {
    fn myfrom(s: CowStr<'t>) -> Self {
        KString::from_ref(s.as_ref())
    }
}

impl<'t> MyFrom<Cow<'t, str>> for KString {
    fn myfrom(s: Cow<'t, str>) -> Self {
        KString::from_ref(s.as_ref())
    }
}

impl MyFrom<usize> for KString {
    fn myfrom(val: usize) -> Self {
        // exact size needed ?
        // let mut buf: [u8; 32] = Default::default();
        // let outp: &mut [u8] = &mut buf;
        // let n = write!(outp, "{}", val).expect("enough space for the formatted number");
        KString::from_string(val.to_string())
    }
}

// impl<'t> MyFrom<HtmlString> for KString {
//     fn myfrom(s: HtmlString) -> Self {
//         let s2 = String::from_utf8(*s)?;
//         KString::from_string(s2)
//     }
// }
// Ah, cannot have errors here. So, do the conversion manually
// outside, please.



// ------------------------------------------------------------------

// / MyFrom must remain owned. Or chaining with MyAsStr as input type
// / leads to non-owned output types combined with owned input
// / types. (Does Rust never want From to lead to non-owned types,
// / BTW?)  So we extend the trait here and add them if not chained
// / with MyAsStr or similar.

// pub trait MyFromInclStr<T> : MyFrom<T> {
//     fn myfrom(s: T) -> Self;    
// }


// impl<'s> MyFromInclStr<&'s str> for &'s str {
//     fn myfrom(s: &'s str) -> Self {
//         s
//     }
// }

// impl<'s> MyFromInclStr<&&'s str> for &'s str {
//     fn myfrom(s: &&'s str) -> Self {
//         *s
//     }
// }

// impl<'s> MyFromInclStr<&'s String> for &'s str {
//     fn myfrom(s: &'s String) -> Self {
//         s
//     }
// }

// impl<'s> MyFromInclStr<&'s KString> for &'s str {
//     fn myfrom(s: &'s KString) -> Self {
//         s
//     }
// }

// impl<'t> MyFromInclStr<&'t CowStr<'t>> for &'t str {
//     fn myfrom(s: &'t CowStr<'t>) -> Self {
//         s
//     }
// }


// Damn. IS THIS WORKABLE now  just deal with no owned inputs?:
// TODO: proper solution.

impl<'s> MyFrom<&'s str> for &'s str {
    fn myfrom(s: &'s str) -> Self {
        s
    }
}

impl<'s> MyFrom<&&'s str> for &'s str {
    fn myfrom(s: &&'s str) -> Self {
        *s
    }
}

impl<'s> MyFrom<&'s String> for &'s str {
    fn myfrom(s: &'s String) -> Self {
        s
    }
}

impl<'s> MyFrom<&'s KString> for &'s str {
    fn myfrom(s: &'s KString) -> Self {
        s
    }
}

impl<'t> MyFrom<&'t CowStr<'t>> for &'t str {
    fn myfrom(s: &'t CowStr<'t>) -> Self {
        s
    }
}


