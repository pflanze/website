use std::{path::{Path, PathBuf}, ffi::OsStr};


// ------------------------------------------------------------------
// Trait for conversion to paths; Into<Box<Path>> does not allow &str.

pub trait IntoBoxPath {
    fn into_box_path(self) -> Box<Path>;
}

impl IntoBoxPath for &str {
    fn into_box_path(self) -> Box<Path> {
        PathBuf::from(self).into()
    }
}
impl IntoBoxPath for String {
    fn into_box_path(self) -> Box<Path> {
        PathBuf::from(self).into()
    }
}
impl IntoBoxPath for PathBuf {
    fn into_box_path(self) -> Box<Path> {
        self.into()
    }
}
impl IntoBoxPath for &PathBuf {
    fn into_box_path(self) -> Box<Path> {
        // self.clone().into() works but might this avoid a copy?:
        (**self).into()
    }
}
impl IntoBoxPath for &Path {
    fn into_box_path(self) -> Box<Path> {
        self.into()
    }
}
impl IntoBoxPath for Box<Path> {
    fn into_box_path(self) -> Box<Path> {
        self
    }
}
impl IntoBoxPath for &Box<Path> {
    fn into_box_path(self) -> Box<Path> {
        self.clone()
    }
}
// ------------------------------------------------------------------

// A path operation that doesn't actually work on Path, bummer. Only
// for strings.

pub fn _base_and_suffix<T: AsRef<[u8]> + ?Sized>(
    s: &T
) -> Option<(&[u8], &str)> {
    let bs: &[u8] = s.as_ref();
    let len = bs.len();
    for (i, c) in bs.iter().rev().enumerate() {
        match c {
            b'/' => return None,
            b'.' => return Some((
                &bs[..(len - i - 1)],
                std::str::from_utf8(&bs[(len - i)..]).unwrap()
            )),
            _  =>
                if ! c.is_ascii_alphanumeric() {
                    return None;
                }
        }
    }
    None
}

pub fn base_and_suffix(s: &str) -> Option<(&str, &str)> {
    let (base, suffix) = _base_and_suffix(s)?;
    Some((std::str::from_utf8(base).unwrap(), suffix))
}

pub fn base(s: &str) -> Option<&str> {
    let (base, _suffix) = _base_and_suffix(s)?;
    Some(std::str::from_utf8(base).unwrap())
}

// Can't do the same for Path/PathBuf, no way to go via bytes. See
// path_replace_extension below



/// Find suffix and if present, return as a &str. Only allows \w
/// characters in suffix.
pub fn suffix<T: AsRef<[u8]> + ?Sized>(
    s: &T
) -> Option<&str> {
    _base_and_suffix(s).and_then(|(_, suffix)| Some(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! t {
        ($e:expr, $r:expr) => {
            assert_eq!(suffix($e), $r);
        }
    }
    
    #[test]
    fn t_suffix() {
        t!("foo", None);
        t!("foo.md", Some("md"));
        t!("foo.", Some("")); // hmm
        t!("foo. md", None);
        t!("foo.md/bar", None);
        t!("foo.md/", None);
        t!("foo.mäd", None);
    }
    #[test]
    fn t_base_and_suffix() {
        assert_eq!(base_and_suffix("foo"), None);
        assert_eq!(base_and_suffix("bar.md"), Some(("bar", "md")));
        assert_eq!(base_and_suffix("foo.md/bar"), None);
        assert_eq!(base_and_suffix("foo.md/bar.md"), Some(("foo.md/bar", "md")));
    }
}

// ------------------------------------------------------------------
            
// XX lib; haven't I done (something like) this already? -- and then
// this one doesn't work, need the below.
// fn path_append<P: AsRef<Path> + AsRef<OsStr>>(base: &P, rel: &P) -> PathBuf {
//     let mut p = PathBuf::from(base);
//     p.push(rel);
//     p
// }
pub fn path_append<P: AsRef<Path>>(base: &Path, rel: &P) -> PathBuf {
    let mut p = PathBuf::from(base);
    p.push(rel);
    p
}

/// Careful, this drops any empty segments, regardless whether at the
/// beginning, end or in the middle. This is useful for search
/// (iterating into a trie), but can't be used as sole information for
/// path operations (e.g. adding paths).
pub fn path_segments<'s>(s: &'s str) -> impl Iterator<Item = &'s str>
{
    s.split('/').filter(|s| !s.is_empty())
}


/// Return the path with the extension (filename suffix) replaced. If
/// it doesn't have an extension, or the extension is different from
/// `orig_extension`, None is returned.
pub fn path_replace_extension<P: AsRef<Path>>(
    p: &P,
    orig_extension: &str,
    new_extension: &str
) -> Option<PathBuf> {
    let p: &Path = p.as_ref();
    let ext = p.extension()?;
    if ext == orig_extension {
        Some(p.with_extension(new_extension))
    } else {
        None
    }
}

// Allocation-less(?) way to compare the extension for Path values
pub fn extension_eq<P: AsRef<Path> + ?Sized,
                    E: AsRef<OsStr> + ?Sized>(
    path: &P, ext: &E
) -> bool {
    // OsStr::try_from( ) interesting, vs. AsRef which never fails?
    // XX Does AsRef allocate?
    let p: &Path = path.as_ref();
    let ext: &OsStr = ext.as_ref();
    p.extension() == Some(ext)
}
    

#[cfg(test)]
mod tests_extension_eq {
    use super::*;

    macro_rules! t {
        ($e:expr, $r:expr) => {
            let r: Option<&str> = $r;
            if let Some(r) = r {
                assert!(extension_eq($e, r));
            } else {
                assert!(! extension_eq($e, "")); // XX oh well.
            }
        }
    }

    // copy-paste from above
    #[test]
    fn t_suffix() {
        t!("foo", None);
        t!("foo.md", Some("md"));
        t!("foo.", Some("")); // hmm
        t!("foo. md", None);
        t!("foo.md/bar", None);
        t!("foo.md/", None);
        t!("foo.mäd", None);
    }
}
