use std::{hash::Hash, collections::{HashMap, hash_map::{Entry, OccupiedEntry}, BTreeMap, btree_map}, borrow::Borrow, time::Duration, fmt::{Display, Debug}, ffi::{OsString, OsStr}, path::PathBuf, fs::create_dir_all, env::VarError};

use anyhow::{Result, anyhow, Context, bail};
use num::CheckedAdd;

/// Return name of the enum value, without the rest of its Debug
/// serialisation. Slower than it needed to be given better ways, but
/// what would those be?
pub fn enum_name<T: Debug>(v: T) -> String {
    let mut s = format!("{v:?}");
    if let Some(i) = s.find(|c| c == '(') {
        s.shrink_to(i);
        s
    } else {
        s
    }
}

/// Return last element of a Vec, first creating it if not present.
pub fn autovivify_last<T>(v: &mut Vec<T>, create: impl FnOnce() -> T) -> &mut T {
    let v : *mut _ = v; // Work around serious rustc deficiency?
    if let Some(last) = unsafe{&mut *v}.last_mut() {
        last
    } else {
        unsafe{&mut *v}.push(create());
        unsafe{&mut *v}.last_mut().unwrap()
    }
}


// XX I have *done* these before
// fn str_skip_while(s: &str, pred: impl Fn(char) -> bool) -> &str {
//     if let Some(i) = s.find(|c| c != '/') {
//         &s[i..]
//     } else {
//         s
//     }
// }

// fn rootbased_to_relative(s: &str) -> &str {
//     str_skip_while(s, |c| c != '/')
// }



// A HashMap::get_mut variant that allows to work around the issue
// that rustc (pre polonius) does not let go of the reference in the
// None case; so we use an Err instead and pick up the reference from
// there.
pub fn hashmap_get_mut<'m, K: Eq + Hash, P: Eq + Hash + ?Sized, V>(
    m: &'m mut HashMap<K, V>,
    k: &P,
) -> Result<&'m mut V,
            &'m mut HashMap<K, V>>
where K: Borrow<P>
{
    let pm: *mut _ = m;
    // Safe because in the true branch we track using the lifetimes
    // (just like in the original get_mut), and in the false branch we
    // just pass the input value.
    if let Some(v) = unsafe{&mut *pm}.get_mut(k) {
        Ok(v)
    } else {
        Err(unsafe{&mut *pm})
    }
}

// Same (see hashmap_get_mut) for BTreeMap.
pub fn btreemap_get_mut<'m, K: Ord, P: Ord + ?Sized, V>(
    m: &'m mut BTreeMap<K, V>,
    k: &P,
) -> Result<&'m mut V,
            &'m mut BTreeMap<K, V>>
where K: Borrow<P>
{
    let pm: *mut _ = m;
    // Safe because in the true branch we track using the lifetimes
    // (just like in the original get_mut), and in the false branch we
    // just pass the input value.
    if let Some(v) = unsafe{&mut *pm}.get_mut(k) {
        Ok(v)
    } else {
        Err(unsafe{&mut *pm})
    }
}


// Modified copy of #[unstable(feature = "map_try_insert", issue =
// "82766")] from
// https://doc.rust-lang.org/src/std/collections/hash/map.rs.html#1132-1137,
// avoiding OccupiedError because that's also unstable. FUTURE:
// replace with try_insert.
pub fn hashmap_try_insert<'m, K: Eq + Hash, V>(
    m: &'m mut HashMap<K, V>,
    key: K,
    value: V
) -> Result<&mut V, OccupiedEntry<K, V>>
{
    match m.entry(key) {
        Entry::Occupied(entry) => Err(entry),
        Entry::Vacant(entry) => Ok(entry.insert(value)),
    }
}


// Modified copy of #[unstable(feature = "map_try_insert", issue =
// "82766")] from
// https://doc.rust-lang.org/src/alloc/collections/btree/map.rs.html#1016-1018;
// see comments on hashmap_try_insert.
pub fn btreemap_try_insert<'m, K: Ord, V>(
    m: &'m mut BTreeMap<K, V>,
    key: K,
    value: V
) -> Result<&'m mut V, btree_map::OccupiedEntry<K, V>>
{
    match m.entry(key) {
        btree_map::Entry::Occupied(entry) => Err(entry),
        btree_map::Entry::Vacant(entry) => Ok(entry.insert(value)),
    }
}


/// Similar to `?` in a context that returns `Option`, this propagates
/// `None` values, but wraps them in `Ok`. I.e. behaves like `?`
/// except if the `Option` context is wrapped in a `Result`.
#[macro_export]
macro_rules! or_return_none {
    ($e:expr) => {{
        let res = $e;
        if let Some(val) = res {
            val
        } else {
            return Ok(None)
        }
    }}
}



// Sigh, for exponential backoff, everybody doing this for themselves?
pub fn duration_mul_div(orig: Duration, multiplier: u64, divider: u64)
                        -> Option<Duration>
{
    let nanos: u64 = orig.as_nanos().checked_mul(multiplier as u128)?
        .checked_div(divider as u128)?
        .try_into().ok()?;
    Some(Duration::from_nanos(nanos))
}


pub fn first<T>(items: &[T]) -> Option<&T> {
    if items.len() > 0 {
        Some(&items[0])
    } else {
        None
    }
}

pub fn rest<T>(items: &[T]) -> Option<&[T]> {
    if items.len() > 0 {
        Some(&items[1..])
    } else {
        None
    }
}

pub fn first_and_rest<T>(items: &[T]) -> Option<(&T, &[T])> {
    if items.len() > 0 {
        Some((&items[0], &items[1..]))
    } else {
        None
    }
}


pub fn debug_stringlikes<S: Display>(v: &[S]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}


/// A loop that caches errors and retries with exponential
/// backoff. (Backoff parameters and error messaging hard coded for
/// now, as is anyhow::Result.)
#[macro_export]
macro_rules! loop_try {
    ( $($body_parts:tt)* ) => {{
        let default_error_sleep_duration = Duration::from_millis(500);
        let mut error_sleep_duration = default_error_sleep_duration;
        loop {
            match (|| -> Result<()> { $($body_parts)* })() {
                Ok(()) => {
                    error_sleep_duration = default_error_sleep_duration;
                }
                Err(e) => {
                    eprintln!("loop_try: got error {e:#}, sleeping for \
                               {error_sleep_duration:?}");
                    thread::sleep(error_sleep_duration);
                    error_sleep_duration =
                        crate::util::duration_mul_div(error_sleep_duration,
                                         1200,
                                         1000)
                        .unwrap_or(default_error_sleep_duration);
                }
            }
        }
    }}
}


#[macro_export]
macro_rules! try_do {
    ( $($b:tt)* ) => ( (|| { $($b)* })() )
}

#[macro_export]
macro_rules! try_result {
    ( $($b:tt)* ) => ( (|| -> Result<_, _> { $($b)* })() )
}

#[macro_export]
macro_rules! try_option {
    ( $($b:tt)* ) => ( (|| -> Option<_> { $($b)* })() )
}


/// A counter. Panics if T wraps around.
pub fn infinite_sequence<T: CheckedAdd + Copy>(
    start: T,
    inc: T
) -> impl FnMut() -> T {
    let mut current = start;
    move || -> T {
        let n = current;
        current = n.checked_add(&inc).expect("number not overflowing");
        n
    }
}

pub fn alphanumber(i: u32) -> String {
    let mut s = Vec::new();
    let mut j: i64 = i.into();
    while j >= 0 {
        s.push(b'a' + ((j % 26) as u8));
        j = (j / 26) - 1;
    }
    s.reverse();
    String::from_utf8(s).expect("all ascii")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_alphanumber() {
        fn t(i: u32, s: &str) {
            assert_eq!(&alphanumber(i), s);
        }
        t(0, "a");
        t(1, "b");
        t(26, "aa");
        t(27, "ab");
        t(27*26-1, "zz");
        t(27*26, "aaa");
        t(27*26+1, "aab");
        t(27*26+26, "aba");
    }
}

pub fn osstring_into_string(s: OsString) -> Result<String> {
    match s.into_string() {
        Ok(s2) => Ok(s2),
        Err(s) => bail!("can't properly decode to string {:?}",
                        s.to_string_lossy()) 
    }
}

pub fn osstr_to_str(s: &OsStr) -> Result<&str> {
    match s.to_str() {
        Some(s2) => Ok(s2),
        None => bail!("can't properly decode to string {:?}",
                      s.to_string_lossy()) 
    }
}

pub fn program_path() -> Result<String> {
    let path = std::env::args_os().into_iter().next().ok_or_else(
            || anyhow!("missing program executable path in args_os"))?;
    osstring_into_string(path).with_context(
        || anyhow!("decoding of program executable path failed"))
}

pub fn program_name() -> Result<String> {
    let path = std::env::args_os().into_iter().next().ok_or_else(
            || anyhow!("missing program executable path in args_os"))?;
    let pb = PathBuf::from(path);
    let fname = pb.file_name().ok_or_else(|| anyhow!("cannot get file name from path {:?}",
                                                     pb.to_string_lossy()))?;
    Ok(osstr_to_str(fname).with_context(
        || anyhow!("cannot decode file name {:?}",
                   fname.to_string_lossy()))?
       .to_string())
}

pub fn log_basedir() -> Result<String> {
    let logbasedir = format!("{}/log/{}",
                             std::env::var("HOME").with_context(
                                 || anyhow!("can't get HOME env var"))?,
                             program_name()?);
    // XX todo: perms / umask!
    create_dir_all(&logbasedir).with_context(
        || anyhow!("can't create log base directory {:?}",
                   logbasedir))?;
    Ok(logbasedir)
}

/// Get an env var as a String; decoding failures are reported as
/// errors. If the var is not set and no fallback was given, an error
/// is reported as well.
pub fn getenv_or(name: &str, fallbackvalue: Option<&str>) -> Result<String> {
    match std::env::var(name) {
        Ok(s) => Ok(s),
        Err(e) => match e {
            VarError::NotPresent =>
                match fallbackvalue {
                    Some(v) => Ok(v.to_string()),
                    None => bail!("{name:?} env var is missing and \
                                   no default provided"),
                },
            VarError::NotUnicode(_) => bail!("{name:?} env var is not unicode"),
        }
    }
}

/// Get an env var as a String; decoding failures are reported as
/// errors.
pub fn getenv(name: &str) -> Result<Option<String>> {
    match std::env::var(name) {
        Ok(s) => Ok(Some(s)),
        Err(e) => match e {
            VarError::NotPresent => Ok(None),
            VarError::NotUnicode(_) => bail!("{name:?} env var is not unicode"),
        }
    }
}

/// Like getenv but reports an error mentioning the variable name if
/// it isn't set.
pub fn xgetenv(name: &str) -> Result<String> {
    getenv(name)?.ok_or_else(
        || anyhow!("missing env var {name:?}"))
}


/// Takes a place (variable or field) holding an `Option<T>` and an
/// expression that returns `T`; returns a `&T` to the value held by
/// the `Option`, runs the expression and stores the result in the
/// place if it holds `None`. The name is from what the `//` operator
/// in Perl (there`s also `//=` which this is really) is called. The
/// expression is executed in the context of the `oerr` call, with no
/// additional subroutine wrapper, meaning e.g. `?` jumps out of the
/// `oerr` (it is designed like this on purpose).
#[macro_export]
macro_rules! oerr {
    { $var:expr, $e:expr } => {
        if let Some(v) = &mut $var {
            v
        } else {
            $var = Some($e);
            $var.as_mut().unwrap()
        }
    }
}

