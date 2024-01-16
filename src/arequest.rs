use std::{net::{SocketAddr, IpAddr}, io::Write, time::SystemTime};

use anyhow::{Result, anyhow};
use blake3::Hasher;
use kstring::KString;
use rouille::{Request, HeadersIter, session::Session};

use crate::{ppath::PPath,
            http_request_method::HttpRequestMethod};

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
    /// A `blake3::Hasher` that has already been filled with some secret data.
    sessionid_hasher: &'h Hasher,
}

impl<'r, 's, 'h> ARequest<'r, 's, 'h> {
    pub fn new(
        request: &'r Request, listen_addr: &'r str, session: &'r Session<'s>,
        sessionid_hasher: &'h Hasher,
    ) -> Result<Self> {
        let path_original = request.url(); // path only
        let path = PPath::from_str(&path_original);
        let path_string = path.to_string();
        // let headers = request.headers();  -- iterator
        let method = HttpRequestMethod::from_str(request.method())?;
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
}
