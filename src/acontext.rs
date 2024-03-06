use std::{net::{SocketAddr, IpAddr}, io::Write, time::SystemTime,
          cell::Cell, borrow::Cow};

use anyhow::{Result, anyhow};
use blake3::Hasher;
use kstring::KString;
use rouille::{Request, HeadersIter,
              session::Session,
              input::priority_header_preferred, Response};

use crate::{ppath::PPath,
            http_request_method::HttpRequestMethod,
            rouille_util::{get_cookie, possibly_add_cookie_header, NewCookieValue},
            language::Language, warn, auri::QueryString, url_encoding::UrlDecodingError};


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
    out: Cell<NewCookieValue<String>>,
}
impl<K: CookieKey> Cookie<K> {
    pub fn new(key: K, got: Option<KString>) -> Self {
        Self {
            key,
            got,
            out: Cell::new(NewCookieValue::Unchanged)
        }
    }

    pub fn key(&self) -> &str {
        self.key.as_str()
    }

    pub fn value(&self) -> &str {
        if let Some(s) = &self.got {
            s.as_str()
        } else {
            self.key.default_value()
        }
    }

    /// Claims the value, afterwards the slot is empty again!
    pub fn take_out_value(&self) -> NewCookieValue<String> {
        self.out.replace(NewCookieValue::Unchanged)
    }

    /// max_age is the Set-Cookie Max-Age value in seconds
    pub fn set(&self, value: String, max_age: i32) {
        self.out.set(NewCookieValue::Updated(value, max_age))
    }

    pub fn clear(&self) {
        self.out.set(NewCookieValue::Deleted);
    }
}


const LANG_COOKIE_MAX_AGE_SECONDS: i32 = 60*60*24*30*2;

pub struct LangKey;
impl CookieKey for LangKey {
    fn as_str(&self) -> &'static str { "lang" }
    fn default_value(&self) -> &'static str { "en" }
}

pub struct AContext<'r, 's, 'h, L: Language> {
    // Fallback for host(): what this server listens on; ip:port or
    // domain:port or whatever is deemed suitable
    listen_addr: &'r str, // ref might be valid for longer but we don't guarantee it
    path: PPath<KString>,
    path_string: String,
    now: SystemTime,
    method: HttpRequestMethod,
    request: &'r Request,
    session: &'r Session<'s>,
    lang: Option<L>,
    // lang_cookie: mostly just for the out bit; `lang` holds the real
    // thing. Private and no setter since the only place where this is
    // set (currently) is in the `new` constructor. Wouldn't even need
    // the interior mutability.
    lang_cookie: Cookie<LangKey>,
    // A `blake3::Hasher` that has already been filled with some secret data.
    sessionid_hasher: &'h Hasher,
}

impl<'r, 's, 'h, L: Language + Default> AContext<'r, 's, 'h, L> {
    pub fn new<F>(
        request: &'r Request, listen_addr: &'r str, session: &'r Session<'s>,
        sessionid_hasher: &'h Hasher,
        lang_from_path: &F,
    ) -> Result<Self>
    where F: Fn(&PPath<KString>) -> Option<L> + Send + Sync
    {
        let path_original = request.url(); // path only
        let path: PPath<KString> = PPath::from_str(&path_original);
        let path_string = path.to_string();
        // let headers = request.headers();  -- iterator
        let method = HttpRequestMethod::from_str(request.method())?;

        let lang_cookie: Option<KString> = get_cookie(request, LangKey.as_str())
            .transpose()?
            .map(KString::from);

        let path_lang: Option<L> = lang_from_path(&path);
        let cookie_lang: Option<L> = lang_cookie.as_ref().and_then(
            |s| L::maybe_from(s.as_str()));
        let browser_lang: Option<L> = request.header("Accept-Language").and_then(|s| {
                let ss = L::strs();
                priority_header_preferred(s, ss.iter().cloned())
                    .map(|i| L::maybe_from(ss[i]).expect("Lang::strs() holds it"))
        });
        let lang: Option<L> = path_lang.or(cookie_lang).or(browser_lang);
        // dbg!(&lang);

        let lang_cookie = Cookie::new(LangKey, lang_cookie);

        // Set cookie, if lang differs from it and clearing the cookie
        // isn't the solution.
        if let Some(langval) = lang {
            if lang != cookie_lang {
                if lang == browser_lang {
                    // don't need a cookie, the default is fine without
                    lang_cookie.clear();
                } else {
                    lang_cookie.set(langval.as_str().into(), LANG_COOKIE_MAX_AGE_SECONDS);
                }
            }
        }

        Ok(AContext {
            listen_addr,
            path,
            path_string,
            now: SystemTime::now(),
            method,
            request,
            session,
            sessionid_hasher,
            lang,
            lang_cookie,
        })
    }
    
    /// Create any response headers that are warranted given the
    /// request or changes applied to self.
    pub fn set_headers(&mut self, headers: &mut Vec<(Cow<'static, str>, Cow<'static, str>)>) {
        possibly_add_cookie_header(headers,
                                   self.lang_cookie.key(),
                                   self.lang_cookie.take_out_value(),
                                   &self.lang_cookie.got);
    }

    /// Like the request part in Apache style Combined Log Format
    pub fn request_line(&self) -> String {
        // `Request` does not appear to maintain the original request
        // line string, thus have to reconstruct it.
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
    pub fn params(&self) -> Result<QueryString, UrlDecodingError>  {
        QueryString::from_str(self.query_string())
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

    pub fn redirect_302_with_query(&self, path: &PPath<KString>) -> Response {
        // XX more testing?  test foo + bar = bar not foo/bar !
        let mut target = self.path().add(path).to_string();
        let querystr = self.request().raw_query_string();
        if ! querystr.is_empty() {
            target.push('?');
            target.push_str(querystr);
        }
        Response::redirect_302(target)
    }

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

    pub fn lang(&self) -> L {
        self.lang.unwrap_or_default()
    }
}

impl<'r, 's, 'h, L: Language> Drop for AContext<'r, 's, 'h, L> {
    fn drop(&mut self) {
        match self.lang_cookie.take_out_value() {
            NewCookieValue::Unchanged => (),
            x => {
                warn!("drop: AContext lang_cookie unused action: {x:?}");
            }
        }
    }
}
