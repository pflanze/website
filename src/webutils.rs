use std::borrow::Cow;
use anyhow::{Result, Error};
use rouille::{Response, ResponseBody};

use ahtml::{Node, AId, HtmlAllocator};
use auri::ppath::PPath;
use chj_util::{nopp as pp, nodt as dt, warn};

use crate::acontext::AContext;
use crate::http_response_status_codes::HttpResponseStatusCode;
use crate::language::Language;
use crate::random_util::randomidstring;


// If thunk returns an Err, we do a nice error box here, still print
// the error to stderr. Also, bump allocator to allow for cases where
// ran out of mem.
pub fn error_boundary<F>(html: &HtmlAllocator, thunk: F) -> AId<Node>
where F: FnOnce() -> Result<AId<Node>>
{
    match thunk() {
        Ok(v) => v,
        Err(e) => {
            let errid = randomidstring().unwrap();
            eprintln!("cap: Error {}: {}", errid, e);
            // Don't actually know if p will be OK!!!
            (|| html.p([],
                       [html.string(format!("An Error happened here (error id {})",
                                            errid))?]))()
                .unwrap() // UH
        }
    }
}


pub fn errorpage_from_status(status: HttpResponseStatusCode) -> Response {
    // XX configure response looks and contents.
    let title = status.title();
    let explanation = status.desc();
    // XX html-escape explanation! (Also, really want to send it?)
    let resp = format!("<html><head><title>{title}</title></head><body><h1>{title}</h1>\
                        <p>{explanation}</p></body></html>\n");
    Response {
        status_code: status.code(),
        headers: vec![(Cow::from("Content-type"), Cow::from("text/html"))],
        data: ResponseBody::from_string(resp),
        upgrade: None, // XX? aha https?
    }
}

pub fn errorpage_from_error(err: Error) -> Response {
    // XX: make status possibly dependent on e instead!
    let status = HttpResponseStatusCode::InternalServerError500;
    // XX show context of course. This MUST provided ALREADY
    eprintln!("ERROR in page (return {status:?}): {err:#}");
    errorpage_from_status(status)
}

pub fn htmlresponse(
    html: &HtmlAllocator,
    status: HttpResponseStatusCode,
    produce: impl for<'a> FnOnce(&HtmlAllocator) -> Result<AId<Node>>
) -> Result<Response>
{
    Ok(Response {
        status_code: status.code(),
        headers: vec![(Cow::from("Content-type"),
                       Cow::from("text/html; charset=utf-8"))],
        data: ResponseBody::from_string(html.to_html_string(produce(html)?, true)),
        upgrade: None, // XX? aha https?
    })
}


/// Resolve a relative path from the current location but fix it up
/// with regards to slash or not slash.  Request `/blog` resolves the
/// relative position `foo/bar` as url `blog/foo/bar`. (HACK? to avoid
/// having to pass up the ancestor parts of paths as they are being
/// resolved in router lookups.)  XX why this here and not just have a method?
pub fn request_resolve_relative<L: Language>(
    context: &AContext<L>, position: PPath<&str>
) -> String {
    assert!(!position.is_absolute());
    let requestpath = context.path(); // path only
    dt!("request_resolve_relative", requestpath, position);
    pp!("request_resolve_relative",
        if requestpath.ends_with_slash() {
            dt!("ends_with_slash");
            position.to_string()
        } else {
            let base = PPath::from_str(requestpath.segments().last().expect(
                "the only way the browser can give us an empty path is \
                 one ending with slash, no?"));
            dt!("base", base);
            // OH, full hack. Right?
            base.as_dir().add(&position).to_string()
        })
}


// Use CowStr ?
pub fn email_url(s: &str) -> String {
    if s.starts_with("mailto:") {
        s.into()
    } else if s.starts_with("https:") || s.starts_with("http:") {
        warn!("using a non-email URL where an email address was expected: {s:?}");
        s.into()
    } else {
        // hope all is well !
        format!("mailto:{s}")
    }
}
