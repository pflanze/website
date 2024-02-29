use std::borrow::Cow;

use kstring::KString;
use rouille::Request;
use rouille::input;

use crate::url_encoding::UrlDecodingError;
use crate::url_encoding::url_decode;
use crate::warn;

#[derive(Debug)]
pub struct RawCookieValue<S>(S)
    where S: AsRef<str>;

/// Get a particular cookie. O(n) with n == number of cookies.
pub fn get_cookie_raw<'r: 's, 's>(
    request: &'r Request, key: &str
) -> Option<RawCookieValue<&'s str>>
{
    input::cookies(request).find(|&(n, _)| n == key).map(|(_, v)| RawCookieValue(v))
}

pub fn get_cookie(request: &Request, key: &str)
                  -> Option<Result<String, UrlDecodingError>> {
    get_cookie_raw(request, key).map(|r| url_decode(r.0))
}


// https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Set-Cookie

#[derive(Debug)]
pub enum NewCookieValue<S: AsRef<str>> {
    /// Want the cookie to be unchanged. So not send any cookie header.
    Unchanged,
    /// Want the cookie to be deleted. Send a cookie header to delete
    /// it if it currently exists.
    Deleted,
    /// Want the cookie to be updated. Send a cookie header to set it;
    /// .0 = val, .1 = Max-Age in seconds. Always send the header, to
    /// update the expiry date.
    // Maximum cookie age is 400 days in Chrome as of August 2022 thus
    // i32 is large enough.
    Updated(S, i32)
}

/// Add a `Set-Cookie` header to `headers` unless `val` is
/// `Unchanged`.  `old_val` is the cookie value the browser sent if
/// any, as a decoded value, i.e. comparable with what string val may
/// contain; if `val` asks to delete and `old_val` says it wasn't
/// present, then nothing is sent. If OTOH both `old_val` and `val`
/// have the same string, it is still sent, just to update the expiry
/// date.
pub fn possibly_add_cookie_header<S: AsRef<str>>(
    headers: &mut Vec<(Cow<'static, str>, Cow<'static, str>)>,
    key: &str,
    val: NewCookieValue<S>,
    old_val: &Option<KString>,
)
{
    let mut add = |s: String| {
        warn!("sending Set-Cookie: {s}");
        headers.push(("Set-Cookie".into(), s.into()));
    };
    match val {
        NewCookieValue::Unchanged => (),
        NewCookieValue::Deleted => {
            let h = format!("{key}=; Max-Age=0; Path=/; HttpOnly");
            if old_val.is_some() {
                add(h)
            }
        }
        NewCookieValue::Updated(s, age) => {
            let h = format!("{key}={}; Max-Age={age}; Path=/; HttpOnly",
                            // XXX escaping? !
                            s.as_ref());
            add(h)
        }
    }
}
