use std::fs::File;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::os::linux::fs::MetadataExt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fmt::Debug, any::type_name, path::PathBuf, borrow::Cow};

use anyhow::{Result, Context, anyhow, bail};
use httpdate::{fmt_http_date, parse_http_date};
use kstring::KString;
use rouille::{Response, extension_to_mime, ResponseBody};
use crate::arequest::AContext;
use crate::ahtml::HtmlAllocator;
use crate::aresponse::AResponse;
use crate::http_request_method::HttpRequestMethodSimple;
use crate::http_response_status_codes::HttpResponseStatusCode;
use crate::language::Language;
use crate::myasstr::MyAsStr;
use crate::ppath::PPath;
use crate::{or_return_none, warn};

// Can't just check `mtime > modsince` since that's ~always true
// because mtime has a nsec value, where modsince has 0
// there. Sigh. fmt_http_date should not accept SystemTime please, and
// SystemTime should have a second accessor or something; huh. And
// can't get at contents at all directly, so can't write tv_sec()
// accessor either.  `modsince.checked_sub(mtime)` doesn't work
// either, takes a Duration as argument. duration_since it is, but
// then have to do it both ways if wanting to know how far off it
// is. Sick.
// fn file_mtime_indicates_file_has_changed(mtime: SystemTime, modsince: SystemTime) -> bool {
// }

// But then, if just wanting to know if the file is *newer* than snapshot time:
 fn file_is_newer_than_snapshot_time(mtime: SystemTime, modsince: SystemTime) -> bool {
     match mtime.duration_since(modsince) {
         Err(_e) => {
             // file is older than snapshot time; client is cheating,
             // or file has been restored to an older version; in any
             // case, it is not newer, so say no
             false
         }
         Ok(secsnewer) => {
             // Make sure it is at least a second newer, due to the
             // rounding issue. Otherwise it would report a fake newer.
             secsnewer >= Duration::from_secs(1)
         }
     }
}




// fn cow<'t1, T: Clone>(
//     v: &T
// ) -> Cow<'t1, &T>
// where Cow<'t1, &'t1 T>: From<&'t1 T>
// {
//     Cow::from(v)
// }

// fn kv<'t1, 't2, T1<'t1>, T2<'t2>>(a: T1<'t1>, b: T2<'t1>) -> (Cow<'t1>, Cow<'t2>) {
//     (Cow::from(a), Cow::from(b))
// }

macro_rules! cow {
    ($a:expr, $b:expr) => {
        (Cow::from($a), Cow::from($b))
    }
}


// XX move to somewhere
fn canonicalize_path<'s, S>(path: &'s [S]) -> Option<Vec<&'s str>>
where S: MyAsStr + 's
{
    let mut out = Vec::new();
    for segment in path {
        let segment = segment.my_as_str();
        match segment {
            "." => (),
            ".." =>
                if out.pop().is_none() {
                    return None
                },
            // Oh, don't forget this one (multiple slashes to one):
            "" => (),
            _ => out.push(segment)
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_canonicalize_path() {
        assert_eq!(canonicalize_path::<&str>(&[]), Some(vec![]));
        assert_eq!(canonicalize_path(&["a", "b"]), Some(vec!["a", "b"]));
        assert_eq!(canonicalize_path(&[".", "a", ".", "b", ".", ".."]),
                   Some(vec!["a"]));
        assert_eq!(canonicalize_path(&["a", "..", "b"]),
                   Some(vec!["b"]));
        assert_eq!(canonicalize_path(&["a", "..", "b", ".."]),
                   Some(vec![]));
        assert_eq!(canonicalize_path(&["a", "..", ".", ".."]),
                   None);
        // Uh, /foo.html/.  is now translated to /foo.html:
        assert_eq!(canonicalize_path(&["a", "foo.html", "."]),
                   Some(vec!["a", "foo.html"]));
        
        // Also, /foo/ to /foo
        assert_eq!(canonicalize_path(&["foo", ""]),
                   Some(vec!["foo"]));
        // Which should be okay for which place that denotes,
        // but problematic with relative path added to it. Sigh.

        assert_eq!(canonicalize_path(&["foo", "", ".", "", "", "a", ".", ""]),
                   Some(vec!["foo", "a"]));
    }
}


pub trait Handler<L: Language>: Debug + Send + Sync {
    /// Returning Ok(None) means, the handler is refusing to handle
    /// the request. It is to be handled as 404 not found by the
    /// caller, unless there's another alternative handler picking up
    /// the request. Err means, the handler has accepted to handle the
    /// request but failed to; this will be handled as internal server
    /// error for now, perhaps have a user-visible error trait in the
    /// future. In either case, the caller has to format an 404 or
    /// other error page; they shall know about the design if
    /// interested.
    fn call<'a>(
        &self,
        request: &AContext<L>,
        method: HttpRequestMethodSimple,
        pathrest: &PPath<KString>,
        html: &HtmlAllocator)
        -> Result<Option<AResponse>>;
}

// /// Allow closures to be used as `Handler`s
// impl<C: Fn(&ARequest,
//            &PPath<KString>,
//            &Allocator) -> Result<Option<Response>>
//      + Debug + Send + Sync>
//     Handler for C
// {
//     fn call<'a>(
//         &self,
//         request: &ARequest,
//         pathrest: &PPath<KString>,
//         html: &Allocator)
//         -> Result<Option<Response>> {
//         self(request, pathrest, html)
//     }
// }
// Can't do with Debug. See `..FnHandler`s below instead.

// ProxyHandler
// FnHandler   or  rlly HtmlDynHandler and PlainDynHandler  and whatnot ?

// ------------------------------------------------------------------
/// Serve files from the local file system
#[derive(Debug)]
pub struct FileHandler {
    /// Path to base directory in local file system from which to
    /// serve the files. No ".." or "." are allowed in the surplus of
    /// the request path.
    basepath: PathBuf,
    // no cache for now
}
impl FileHandler {
    pub fn new(basepath: impl Into<PathBuf>) -> FileHandler {
        FileHandler {
            basepath: basepath.into()
        }
    }
}

impl<L: Language + Default> Handler<L> for FileHandler {
    /// Returns None if the file does not exist
    fn call<'a>(
        &self,
        request: &AContext<L>,
        method: HttpRequestMethodSimple,
        pathrest: &PPath<KString>,
        _html: &HtmlAllocator)
        -> Result<Option<AResponse>> {
        if method.is_post() {
            bail!("can't POST to a file")
        }
        let canonpath = or_return_none!(canonicalize_path(pathrest.segments()));
        if canonpath.is_empty() {
            return Ok(None) // Since it's a directory, not a file.
                // Todo: directory indices, but as a separate handler
        }
        let canonpathstr: String = canonpath.join("/");
        let full_path: PathBuf = self.basepath.join(&canonpathstr);
        // XX would we need better than extension based mime type
        // matching?

        // XX instead do File::open first and then get metadata from
        // the fh: *does* this work (portably?) for directories, too?
        let metadata =
            match full_path.metadata() {
                Ok(m) => m,
                Err(e) =>
                    match e.kind() {
                        ErrorKind::NotFound => return Ok(None),
                        _ => return Err(e).with_context(
                            || anyhow!("can't open file for reading: {:?}",
                                       full_path))
                    }
            };

        if metadata.is_dir() {
            warn!("is_dir, not handling dirs yet");
            Ok(None)
        } else if metadata.is_symlink() {
            warn!("is_symlink, not handling symlinks yet");
            Ok(None)
        } else if metadata.is_file() {
            let mimetype = 
                if let Some(extension_os) = full_path.extension() {
                    let extension = extension_os.to_str().expect("came from String above");
                    extension_to_mime(extension)
                } else {
                    "text/plain" // XX ?
                };
            match File::open(&full_path) {
                Err(e) =>
                    match e.kind() {
                        ErrorKind::NotFound => Ok(None),
                        _ => Err(e).with_context(
                            || anyhow!("can't open file for reading: {:?}",
                                       full_path))?
                    },
                Ok(fh) => {
                    let mtime: SystemTime = metadata.modified()?;
                    let age: Duration = mtime.elapsed()?;
                    let age_seconds = age.as_secs() as u128;
                    let age_allowed = age_seconds + age_seconds / 10;
                    let age_allowed_duration: Duration = Duration::new(age_allowed as u64, 0);
                    let expires = mtime.checked_add(age_allowed_duration).ok_or_else(
                        || anyhow!("time overflow??"))?;
                    let mtime_seconds = mtime.duration_since(UNIX_EPOCH)?.as_secs();
                    let etag_quoted = format!("{:?}", mtime_seconds.to_string());

                    let headers = vec![
                        cow!("Content-type", mimetype),
                        cow!("Last-Modified", fmt_http_date(mtime)),

                        // The Content-Length header is dropped again! No point adding it.
                        // cow!("Content-Length", metadata.st_size().to_string()),

                        // https://developer.mozilla.org/en-US/docs/Web/HTTP/Caching
                        // HTTP caching - HTTP MDN.html
                        cow!("Cache-Control",
                             format!("max-age={}", age_allowed)),
                        // And also add Expires, even though it hasn't
                        // changed anything for Firefox issue either.
                        cow!("Expires", fmt_http_date(expires)),

                        // https://webmasters.stackexchange.com/questions/63119/why-doesnt-firefox-cache-my-javascript-file
                        // iis - Why doesn't FireFox cache my JavaScript file - Webmasters Stack Exchange.html
                        cow!("ETag", etag_quoted.clone()),
                    ];
                    let send_file = |headers| {
                        Ok(Some(Response {
                            status_code:
                            HttpResponseStatusCode::OK200.code(),
                            headers,
                            data: ResponseBody::from_reader_and_size(
                                fh,
                                // XX dangerous re panics?
                                metadata.st_size() as usize),
                            upgrade: None, // XX
                        }.into()))
                    };
                    let send_notmodified = |headers| {
                        Ok(Some(Response {
                            status_code:
                            HttpResponseStatusCode::NotModified304.code(),
                            // Still send these headers? --
                            // Yes, let the client know that
                            // the file might even be *older*
                            // than what it saw?
                            headers,
                            data: ResponseBody::empty(),
                            upgrade: None, // XX
                        }.into()))
                    };
                    if let Some(modsince_str) = request.header("If-Modified-Since")
                    {
                        let modsince = parse_http_date(modsince_str).with_context(
                            || anyhow!("parsing If-Modified-Since {:?}",
                                       modsince_str))?;
                        if file_is_newer_than_snapshot_time(mtime, modsince) {
                            warn!("If-Modified-Since: {}; sending it", modsince_str);
                            send_file(headers)
                        } else {
                            warn!("If-Modified-Since: {}; NotModified304", modsince_str);
                            send_notmodified(headers)
                        }
                    } else if let Some(nonematch_str) = request.header("If-None-Match") {
                        warn!("got If-None-Match {nonematch_str:?}, \
                               compared to calculated {etag_quoted:?}");
                        if nonematch_str == etag_quoted {
                            send_notmodified(headers)
                        } else {
                            send_file(headers)
                        }
                    } else {
                        send_file(headers)
                    }
                }
            }
        } else {
            warn!("neither file nor symlink nor dir: device file or fifo or socket?");
            Ok(None)
        }
    }
}


// ------------------------------------------------------------------
/// A Handler that allows a path surplus, passing it to the handler
/// Fn. The handler may still refuse to handle the request (404).
#[derive(Clone, Copy)]
pub struct FnHandler<L, F>
where L: Language,
      F: Fn(&AContext<L>, HttpRequestMethodSimple, &PPath<KString>, &HtmlAllocator)
            -> Result<Option<AResponse>> + Send + Sync
{
    phantom: PhantomData<L>,
    handler: F
}

impl<L: Language,
     F: Fn(&AContext<L>, HttpRequestMethodSimple, &PPath<KString>, &HtmlAllocator)
           -> Result<Option<AResponse>> + Send + Sync>
    FnHandler<L, F>
{
    pub fn new(handler: F) -> Self {
        Self {
            phantom: PhantomData,
            handler,
        }
    }
}

impl<L: Language + Send + Sync,
     F: Fn(&AContext<L>, HttpRequestMethodSimple, &PPath<KString>, &HtmlAllocator)
           -> Result<Option<AResponse>> + Send + Sync>
    Handler<L> for FnHandler<L, F>
{
    fn call(
        &self,
        request: &AContext<L>,
        method: HttpRequestMethodSimple,
        pathrest: &PPath<KString>,
        html: &HtmlAllocator) -> Result<Option<AResponse>>
    {
        (self.handler)(request, method, pathrest, html)
    }
}

impl<L: Language,
     F: Fn(&AContext<L>, HttpRequestMethodSimple, &PPath<KString>, &HtmlAllocator)
           -> Result<Option<AResponse>> + Send + Sync>
    Debug for FnHandler<L, F>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("FnHandler({})",
                                 type_name::<F>()))
    }
}

// ------------------------------------------------------------------
/// A Handler that does not allow a path surplus, passing it to the handler Fn.
#[derive(Clone, Copy)]
pub struct ExactFnHandler<L, F>
where L: Language,
      F: Fn(&AContext<L>, HttpRequestMethodSimple, &HtmlAllocator)
            -> Result<AResponse> + Send + Sync
{
    phantom: PhantomData<L>,
    handler: F
}

impl<L: Language + Send + Sync,
     F: Fn(&AContext<L>, HttpRequestMethodSimple, &HtmlAllocator)
           -> Result<AResponse> + Send + Sync>
    ExactFnHandler<L, F>
{
    pub fn new(handler: F) -> Self {
        Self {
            phantom: PhantomData,
            handler,
        }
    }
}

impl<L: Language + Send + Sync,
     F: Fn(&AContext<L>, HttpRequestMethodSimple, &HtmlAllocator)
           -> Result<AResponse> + Send + Sync>
    Handler<L>
    for ExactFnHandler<L, F>
{
    fn call(
        &self,
        request: &AContext<L>,
        method: HttpRequestMethodSimple,
        pathrest: &PPath<KString>,
        html: &HtmlAllocator) -> Result<Option<AResponse>>
    {
        if pathrest.segments().is_empty() {
            Ok(Some((self.handler)(request, method, html)?))
        } else {
            // refuse to handle if there is a rest (-> 404)
            Ok(None)
        }
    }
}

impl<L: Language,
     F: Fn(&AContext<L>, HttpRequestMethodSimple, &HtmlAllocator)
           -> Result<AResponse> + Send + Sync>
    Debug
    for ExactFnHandler<L, F>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("FnHandler({})",
                                 type_name::<F>()))
    }
}


// ------------------------------------------------------------------
// Redirect handler

pub fn map_redirect(code: HttpResponseStatusCode) -> Option<Box<dyn Fn(String) -> Response>>
{
    match code {
        HttpResponseStatusCode::MovedPermanently301 => Some(Box::new(Response::redirect_301)),
        HttpResponseStatusCode::Found302 => Some(Box::new(Response::redirect_302)),
        // ^ Instruct the client to do GET
        HttpResponseStatusCode::SeeOther303 => Some(Box::new(Response::redirect_303)),
        HttpResponseStatusCode::TemporaryRedirect307 => Some(Box::new(Response::redirect_307)),
        // ^ Instruct the client to do GET or POST as per original request
        HttpResponseStatusCode::PermanentRedirect308 => Some(Box::new(Response::redirect_308)),
        _ => None
    }
}

pub struct RedirectHandler<L, F>
where L: Language,
      F: Fn(&AContext<L>) -> String + Send + Sync,
{
    // Phantom is necessary because L is only used via impl in F; and
    // I want F to be bound here already, not just in the
    // methods. (Because indirect, Rust doesn't otherwise tie the two?)
    phantom: PhantomData<L>,
    calculate_target: F,
    code: HttpResponseStatusCode,
}

impl<L, F> RedirectHandler<L, F>
where L: Language,
      F: Fn(&AContext<L>) -> String + Send + Sync,
{
    /// Panics immediately when given a `code` that's not a redirect.
    pub fn new(calculate_target: F, code: HttpResponseStatusCode) -> Self {
        let _ = map_redirect(code).expect(
            "given code must be a redirect");
        RedirectHandler {
            phantom: PhantomData,
            calculate_target,
            code,
        }
    }
}

impl<L, F> Debug for RedirectHandler<L, F>
where L: Language,
      F: Fn(&AContext<L>) -> String + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("RedirectHandler(?, {:?})", self.code))
    }
}

impl<L, F> Handler<L> for RedirectHandler<L, F>
where L: Language  + Send + Sync,
      F: Fn(&AContext<L>) -> String + Send + Sync,
{
    fn call<'a>(
        &self,
        request: &AContext<L>,
        _method: HttpRequestMethodSimple,
        _pathrest: &PPath<KString>,
        _html: &HtmlAllocator
    ) -> Result<Option<AResponse>> {
        let target = (self.calculate_target)(request);
        let responder = map_redirect(self.code).expect("already checked earlier");
        Ok(Some(responder(target).into()))
    }
}
