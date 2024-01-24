use std::{net::{SocketAddr, IpAddr}, io::Write, time::SystemTime,
          cell::Cell};

use anyhow::{Result, anyhow};
use blake3::Hasher;
use kstring::KString;
use rouille::{Request, HeadersIter, session::Session, input::priority_header_preferred};

use crate::{ppath::PPath,
            http_request_method::HttpRequestMethod, rouille_util::get_cookie, lang::Lang};


pub trait CookieKey {
    fn as_str(&self) -> &'static str;
    fn default_value(&self) -> &'static str;
}

/// The present and desired cookie state for a key and val.
pub struct Cookie<K: CookieKey>{
    key: K,
    // From request; e.g. URL info (/en/..) overriding cookie value
    // for K==CookieKey overriding browser language setting.
    got: Option<KString>,
    // Cookie to be set in response. (OnceCell is unstable, hence use Cell)
    out: Cell<Option<String>>,
}
impl<K: CookieKey> Cookie<K> {
    pub fn new(key: K, got: Option<KString>) -> Self {
        Self {
            key,
            got,
            out: Cell::new(None)
        }
    }

    pub fn value(&self) -> &str {
        if let Some(s) = &self.got {
            s.as_str()
        } else {
            self.key.default_value()
        }
    }
}


pub struct LangKey;
impl CookieKey for LangKey {
    fn as_str(&self) -> &'static str { "lang" }
    fn default_value(&self) -> &'static str { "en" }
}

/// todo: rename to AContext, it has more than request data now.
pub struct ARequest<'r, 's, 'h> {
    // Fallback for host(): what this server listens on; ip:port or
    // domain:port or whatever is deemed suitable
    listen_addr: &'r str, // ref might be valid for longer but we don't guarantee it
    path_original: String,
    path: PPath<KString>,
    path_string: String,
    now: SystemTime,
    method: HttpRequestMethod,
    request: &'r Request,
    session: &'r Session<'s>,
    lang: Option<Lang>,
    lang_cookie: Cookie<LangKey>, // mostly just for the out bit; `lang` holds the real thing
    // A `blake3::Hasher` that has already been filled with some secret data.
    sessionid_hasher: &'h Hasher,
}

impl<'r, 's, 'h> ARequest<'r, 's, 'h> {
    pub fn new(
        request: &'r Request, listen_addr: &'r str, session: &'r Session<'s>,
        sessionid_hasher: &'h Hasher,
    ) -> Result<Self> {
        let path_original = request.url(); // path only
        let path: PPath<KString> = PPath::from_str(&path_original);
        let path_string = path.to_string();
        // let headers = request.headers();  -- iterator
        let method = HttpRequestMethod::from_str(request.method())?;
        let lang_cookie = get_cookie(request, LangKey.as_str()).map(KString::from_ref);
        let lang: Option<Lang> =
            path.segments().get(0).and_then(|s| Lang::maybe_from(s.as_str()))
            .or_else(|| lang_cookie.as_ref().and_then(|s| Lang::maybe_from(s.as_str())))
            .or_else(|| request.header("Accept-Language").and_then(|s| {
                let ss = Lang::strs();
                priority_header_preferred(s, ss.iter().cloned())
                    .map(|i| Lang::maybe_from(ss[i]).expect("Lang::strs() holds it"))
            }));
        // dbg!(&lang);
        Ok(ARequest {
            listen_addr,
            path_original,
            path,
            path_string,
            now: SystemTime::now(),
            method,
            request,
            session,
            sessionid_hasher,
            lang,
            lang_cookie: Cookie::new(LangKey, lang_cookie),
        })
    }

    /// Like the request part in Apache style Combined Log Format
    pub fn request_line(&self) -> String {
        // Request does not appear to maintain the original request
        // line; thus have to reconstruct it, bummer.
        format!("{} {}",
                self.request.method(),
                self.request.raw_url())
    }
    /// `foo` part in `?foo`
    pub fn query_string(&self) -> &str {
        self.request.raw_query_string()
    }
    pub fn user_agent(&self) -> Option<&str> {
        self.request.header("user-agent")
    }
    pub fn client_ip(&'r self) -> IpAddr {
        self.request.remote_addr().ip()
    }
    pub fn is_secure(&'r self) -> bool {
        self.request.is_secure()
    }
    pub fn method_str(&'r self) -> &'r str { self.request.method() }
    /// None indicates invalid/unknown method; use `method_str` to
    /// get the original string.
    pub fn method(&self) -> HttpRequestMethod { self.method }
    pub fn is_post(&self) -> bool {
        self.method.is_post()
    }
    /// Only checks query parameters! For `POST` data, use
    /// [`post_input!`](https://docs.rs/rouille/latest/rouille/input/post/index.html)
    pub fn get_param(&self, name: &str) -> Option<String>  {
        self.request.get_param(name)
    }
    pub fn param(&self, name: &str) -> Result<String>  {
        self.get_param(name).ok_or_else(
            || anyhow!("missing param {name:?}"))
    }
    pub fn host(&self) -> Option<&str> { self.request.header("host") }
    pub fn host_or_listen_addr(&self) -> &str {
        self.request.header("host").unwrap_or(&self.listen_addr)
    }
    pub fn client_addr(&'r self) -> &'r SocketAddr { self.request.remote_addr() }
    pub fn path(&self) -> &PPath<KString> { &self.path }
    pub fn path_str(&self) -> &str { &self.path_string }
    pub fn now(&self) -> &SystemTime { &self.now }
    pub fn referer(&self) -> Option<&str> {
        self.header("referer")
    }

    pub fn header(&self, key: &str) -> Option<&str> { self.request.header(key) }
    pub fn headers(&self) -> HeadersIter { self.request.headers() }

    pub fn request(&self) -> &Request { self.request }
    pub fn session(&self) -> &Session { &self.session }
    pub fn session_id(&self) -> &str { self.session.id() }

    pub fn writeln(&self, outp: &mut impl Write) -> Result<()> {
        writeln!(outp, "{:?}: {:?} {:?} / {:?} ({:?})",
                 self.client_addr(), self.method_str(), self.host(),
                 self.path(), self.headers())?;
        Ok(())
    }

    pub fn sessionid_hasher(&self) -> Hasher {
        self.sessionid_hasher.clone()
    }

    pub fn lang(&self) -> Lang {
        self.lang.unwrap_or_default()
    }
}
