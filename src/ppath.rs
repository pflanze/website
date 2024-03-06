//! Paths independent of the local file system (pure
//! functions). E.g. for use in web applications.

//! Does not (currently, anyway) concern itself with handling ".." or
//! ".", i.e. does not offer canonicalization.--change?

use std::fmt::Debug;

use anyhow::{Result, bail};

use crate::{path::path_segments, util::{rest, first}, myasstr::MyAsStr, myfrom::MyFrom};

#[derive(Clone, Debug, PartialEq)]
pub struct PPath<Segment: Clone + Debug> {
    is_absolute: bool,
    ends_with_slash: bool,
    segments: Vec<Segment>, // without empty ones
}

// aww hell never works so give up. Problem is ownership can be for
// S. Which vanishes. Although, MyFrom only has owned results? sooooooo?

// impl<'s, T> PPath<T>
// where T: MyFrom<&'s str> + MyAsStr + Clone + Debug + 's
// {
//     pub fn from_myasstr<S>(s: S) -> Self
//     where S: MyAsStr + 's
//     {
//         // COPYPASTE from from_str
//         let s_str = s.my_as_str();
//         // XX allow the empty string?
//         let is_absolute = s_str.chars().next() == Some('/');
//         let ends_with_slash = s_str.chars().last() == Some('/');
//         PPath {
//             is_absolute,
//             ends_with_slash,
//             segments: path_segments(s_str).map(|v| T::myfrom(v)).collect()
//         }
//     }
// }

impl<'s, T> PPath<T>
where T: MyFrom<&'s str> + Clone + Debug + 's
{
    pub fn from_str(s: &'s str) -> Self
    {
        // COPYPASTE from from_str
        // XX allow the empty string?
        let is_absolute = s.chars().next() == Some('/');
        let ends_with_slash = s.chars().last() == Some('/');
        PPath {
            is_absolute,
            ends_with_slash,
            segments: path_segments(s).map(|v| T::myfrom(v)).collect()
        }
    }
}

fn repeated_dotdot<'t, T>(n: usize) -> Vec<T>
where T: From<&'t str> + Clone
{
    std::iter::repeat(T::from("..")).take(n).collect()
}

impl<'s, T> PPath<T>
where T: From<&'s str> + MyAsStr + Clone + Debug
{
    pub fn to_string(&self) -> String {
        let mut s = String::new();
        if self.is_absolute {
            s.push('/');
        }
        if self.segments.is_empty() {
            if ! self.is_absolute {
                s.push('.'); // XX first time we use "." !
                if self.ends_with_slash {
                    s.push('/');
                }
            }
        } else {
            let mut seen = false;
            for p in &self.segments {
                if seen {
                    s.push('/');
                }                    
                s.push_str(p.my_as_str());
                seen = true;
            }
            if self.ends_with_slash {
                s.push('/');
            }
        }
        s
    }

    /// Returns a relative path, that when added to base yields
    /// self. Both path must either be absolute or relative. If base
    /// is not marked with ends_with_slash, the last segment is
    /// dropped before adding self (like a web browser works).
    pub fn sub(&self, base: &Self) -> Result<Self>
    {
        // XX check for both to be canonical, too!
        if self.is_absolute == base.is_absolute {
            let mut ss = self.segments.iter();
            let base_segments = base.segments();
            let mut bs =
                if base.ends_with_slash || base_segments.is_empty() {
                    base_segments
                } else {
                    // if base_segments.is_empty() {
                    //     bail!("base path is empty and does not end with a slash")
                    // }
                    &base_segments[0..base_segments.len() - 1]
                }.iter();
            loop {
                let s = ss.next();
                let b = bs.next();
                match (s, b) {
                    (Some(s), Some(b)) =>  {
                        let s_str = s.my_as_str();
                        let b_str = b.my_as_str();
                        if s_str != b_str {
                            let mut v = repeated_dotdot(bs.count() + 1);
                            v.push(s.clone());
                            v.extend(ss.cloned());
                            return Ok(PPath {
                                is_absolute: false,
                                ends_with_slash: self.ends_with_slash,
                                segments: v,
                            });
                        }
                    }
                    (Some(s), None) => {
                        let mut v = vec![s.clone()];
                        v.extend(ss.cloned());
                        return Ok(PPath {
                            is_absolute: false,
                            ends_with_slash: self.ends_with_slash,
                            segments: v,
                        });
                    }
                    (None, Some(_b)) => {
                        let v = repeated_dotdot(bs.count() + 1);
                        return Ok(PPath {
                            is_absolute: false,
                            ends_with_slash: self.ends_with_slash,
                            segments: v,
                        });
                    }
                    (None, None) => {
                        // XX return empty segments, really?
                        return Ok(PPath {
                            is_absolute: false,
                            ends_with_slash: self.ends_with_slash,
                            segments: vec![],
                        });
                    }
                }
            }
            
        } else {
            bail!("minus_base: the paths are not both absolute or relative")
        }
    }

    /// True if there are either `.` nor `..` segments.
    pub fn contains_dot_or_dotdot(&self) -> bool {
        self.segments.iter().any(
            |s| {
                match s.my_as_str() {
                    "." => true,
                    ".." => true,
                    _ => false
                }
            })
    }

    /// True if there are neither `.` nor `..` segments.
    pub fn is_canonical(&self) -> bool {
        ! self.contains_dot_or_dotdot()
    }

    /// More efficient than parsing `other` into a `PPAth` and
    /// comparing afterwards, and ignores differences on is_absolute
    /// and ends_with_slash!
    pub fn same_document_as_path_str(&self, other: &str) -> bool {
        itertools::equal(self.segments.iter().map(|v| v.my_as_str()),
                         path_segments(other))
    }

}

// fn check_non_canonical<P: Clone + Debug + PartialEq>(
// oh, requires &str. So do track canonical instead.

impl<P: Clone + Debug> PPath<P> {
    pub fn new(is_absolute: bool,
               ends_with_slash: bool,
               segments: Vec<P>
    ) -> Self {
        PPath { is_absolute, ends_with_slash, segments }
    }
    pub fn is_absolute(&self) -> bool {
        self.is_absolute
    }
    pub fn ends_with_slash(&self) -> bool {
        self.ends_with_slash
    }
    /// without empty ones
    pub fn segments(&self) -> &[P] {
        &self.segments
    }

    /// Get the path explicitly as a path to a directory, i.e. sets
    /// ends_with_slash.
    pub fn as_dir(&self) -> Self {
        PPath { is_absolute: self.is_absolute,
                ends_with_slash: true,
                segments: self.segments.clone() }
    }

    pub fn add_segments(&self, segments: &[P], ends_with_slash: bool) -> Self {
        // segments is never absolute. OK?
        let mut newsegments =
            if self.ends_with_slash {
                self.segments.clone()
            } else {
                self.segments[0..self.segments.len() - 1]
                    .iter().map(|c| (*c).clone()).collect()
            };
        newsegments.extend_from_slice(segments);
        PPath {
            is_absolute: self.is_absolute,
            ends_with_slash,
            segments: newsegments
        }
        
    }

    pub fn add(&self, other: &Self) -> Self {
        if other.is_absolute {
            (*other).clone()
        } else {
            self.add_segments(&other.segments, other.ends_with_slash)
        }
    }

    pub fn into_add(self, other: Self) -> Self {
        if other.is_absolute {
            other
        } else {
            // XX future: could optimize by absorbing the vectors
            // (avoiding Clone altogether, actually!)
            self.add(&other)
        }
    }

    pub fn first(&self) -> Option<P> {
        // XX What does that operation mean? Does absolute
        // etc. matter?
        first(&self.segments).cloned()
    }

    pub fn rest(&self) -> Option<Self> {
        // XX really allow rest on absolut paths? What does that
        // operation mean?
        Some(PPath {
            is_absolute: false,
            ends_with_slash: self.ends_with_slash,
            segments: rest(&self.segments)?.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_add() {
        let paths = [
            PPath::from_str(""), // 0
            PPath::from_str("/"), // 1
            PPath::from_str("/hello"), // 2
            PPath::from_str("/world/"), // 3
            PPath::from_str("foo"), // 4
            PPath::from_str("bar/baz/"), // 5
            PPath::from_str("foo/hum"), // 6
            PPath::from_str("foo/hum/"), // 7
        ];
        // assert_eq!(
        //     paths[0].add(&paths[1]),
        //     PPath { is_absolute: , ends_with_slash: , segments: vec![] });
        // fn t(p: PPath<&str>) -> (PPath<&str>, String) {
        //     (p, p.to_string())
        // }
        let t = |p0: usize, p1: usize| {
            let p: PPath<&str> = paths[p0].add(&paths[p1]);
            let s = p.to_string();
            (p, s)
        };
        assert_eq!(
            t(1, 2),
            (PPath { is_absolute: true, ends_with_slash: false,
                     segments: vec!["hello"] },
             String::from("/hello")));
        assert_eq!(
            t(2, 4),
            (PPath { is_absolute: true, ends_with_slash: false,
                     segments: vec!["foo"] },
            String::from("/foo")));
        assert_eq!(
            t(3, 4),
            (PPath { is_absolute: true, ends_with_slash: false,
                     segments: vec!["world", "foo"] },
            String::from("/world/foo")));
        assert_eq!(
            t(2, 5),
            (PPath { is_absolute: true, ends_with_slash: true,
                     segments: vec!["bar", "baz"] },
            String::from("/bar/baz/")));
        assert_eq!(
            t(4, 5),
            (PPath { is_absolute: false, ends_with_slash: true,
                     segments: vec!["bar", "baz"] },
             String::from("bar/baz/")));
        assert_eq!(
            t(6, 5),
            (PPath { is_absolute: false, ends_with_slash: true,
                     segments: vec!["foo", "bar", "baz"] },
             String::from("foo/bar/baz/")));
        assert_eq!(
            t(7, 5),
            (PPath { is_absolute: false, ends_with_slash: true,
                     segments: vec!["foo", "hum", "bar", "baz"] },
             String::from("foo/hum/bar/baz/")));
    }

    #[test]
    fn t_minus_base() {
        let minus = |a, b| -> String {
            match PPath::<&str>::from_str(a).sub(&PPath::from_str(b)) {
                Ok(p) => p.to_string(),
                Err(e) => String::from("ERR: ") + &e.to_string()
            }
        };
        assert_eq!(minus("/a", "/"),  "a");
        assert_eq!(minus("/a", ""),
                   "ERR: minus_base: the paths are not both absolute or relative");
        assert_eq!(minus("a", ""),
                   "a"
                   // "ERR: base path is empty and does not end with a slash"
        );
        assert_eq!(minus("a/b/", ""),
                   "a/b/"
                   // "ERR: base path is empty and does not end with a slash"
        );
        assert_eq!(minus("a/b/", "a"),  "a/b/");
        assert_eq!(minus("a/b/", "a/"),  "b/");
        assert_eq!(minus("a/b/", "c"),  "a/b/");
        assert_eq!(minus("a/b/", "c/d"),  "../a/b/");
        assert_eq!(minus("a/b/", "c/d/"),  "../../a/b/");
        assert_eq!(minus("c/b/", "c/d/"),  "../b/");
        assert_eq!(minus("/a/b/", "/c/d"),  "../a/b/");
        assert_eq!(minus("/a/b/", "/c/d/"),  "../../a/b/");
        assert_eq!(minus("/c/b/", "/c/d/"),  "../b/");
        assert_eq!(minus("a/", "a/"),  "./");
        assert_eq!(minus("a", "a/"),  ".");
        assert_eq!(minus("a/", "a"),  "a/");
        assert_eq!(minus("a/", "a/b/c"),  "../");
        assert_eq!(minus("a", "a/b/c"),  "..");
        assert_eq!(minus("a", "a/b/c/"),  "../..");
        assert_eq!(minus("/blog", "/blog/2023/10/22/foo.html"),  "../../..");
        // ^ Wow, *Firefox* adds the slash to the end of the target
        // url in this case!
    }

    #[test]
    fn t_canonical() {
        let canon = |s| -> bool {
            PPath::<&str>::from_str(s).is_canonical()
        };
        assert!(canon("a///b/c.html"));
        assert!(canon("c.html"));
        assert!(canon("")); // XXX ?
        assert!(! canon("."));
        assert!(! canon("./a"));
        assert!(! canon("a//./b/c.html"));
        assert!(! canon("a//../c.html"));
    }
}
