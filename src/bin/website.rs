use std::sync::Arc;
use std::thread;
use blake3::Hasher;
use kstring::KString;
use scoped_thread_pool;
use website::access_control::db::access_control_transaction;
use website::access_control::statements_and_methods::DO_WARN_THREAD;
use website::alist::AList;
use website::apachelog::Logs;
use website::arequest::ARequest;
use website::blog::Blog;
use website::ahtml::{AllocatorPool, Flat, HtmlAllocator, Node, att};
use anyhow::{Result, bail, anyhow};
use website::hostrouter::{HostRouter, HostsRouter};
use website::http_response_status_codes::HttpResponseStatusCode;
use website::imageinfo::static_img;
use website::io_util::my_read_to_string;
use website::language::Language;
use website::path::base_and_suffix;
use website::ppath::PPath;
use website::style::footnotes::{WikipediaStyle, BlogStyle};
use website::handler::{ExactFnHandler, RedirectHandler};
use website::handler::FileHandler;
use lazy_static::lazy_static;
use website::markdown::StylingInterface;
use rouille::Server;
use website::nav::{Nav, NavEntry, SubEntries};
use website::router::MultiRouter;
use website::util::{log_basedir, getenv_or, getenv, xgetenv};
use website::webparts::{markdownpage_handler, blog_handler, server_handler,
                        login_handler, Restricted, markdowndir_handler};
use website::website_layout::WebsiteLayout;
use website::handler::Handler;
use website::{website_benchmark, warn};


// ------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Lang {
    En,
    De,
}

impl Language for Lang {
    // XX use some parse trait instead ?

    fn maybe_from(s: &str) -> Option<Self> {
        match dbg!(s) {
            "en" => Some(Lang::En),
            "de" => Some(Lang::De),
            _ => None
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::De => "de",
        }
    }

    fn members() -> &'static [Self] {
        &[Lang::En, Lang::De]
    }

    fn strs() -> &'static [&'static str] {
        &["en", "de"]
    }
}

impl Default for Lang {
    fn default() -> Self {
        Lang::En
    }
}

impl From<&str> for Lang {
    fn from(s: &str) -> Self {
        Lang::maybe_from(s).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_from() {
        assert_eq!(Lang::from("de"), Lang::De);
        assert_eq!(Lang::from("de_CH"), Lang::De);
        assert_eq!(Lang::from("de-CH"), Lang::De);
        assert_eq!(Lang::from("dee"), Lang::De);
        assert_eq!(Lang::from("dfe"), Lang::En);
        assert_eq!(Lang::maybe_from("dfe"), None);
        assert_eq!(Lang::maybe_from("d"), None);
    }
}
// ------------------------------------------------------------------

// HACK for now
const LANG_FROM_PATH: &[(&str, (Lang, &str))] = &[
    // basename, is lang, has sibling
    ("en", (Lang::En, "de")),
    ("climate", (Lang::En, "umwelt")),
    ("contact", (Lang::En, "kontakt")),
    ("projects", (Lang::En, "projekte")),
    ("de", (Lang::De, "en")),
    ("umwelt", (Lang::De, "climate")),
    ("projekte", (Lang::De, "projects")),
    ("kontakt", (Lang::De, "contact")),
];

const NAV: &[(Lang, Nav)] = &[
    (Lang::En, Nav(&[
        NavEntry {
            name: "Home",
            path: "/en.html",
            subentries: SubEntries::Static(&[]),
        },
        NavEntry {
            name: "Climate & Environment",
            path: "/climate.html",
            subentries: SubEntries::Static(&[]),
        },
        NavEntry {
            name: "Projects",
            path: "/projects.html",
            subentries: SubEntries::Static(&[]),
        },
        NavEntry {
            name: "Contact",
            path: "/contact.html",
            subentries: SubEntries::Static(&[]),
        },
        // NavEntry {
        //     name: "Blog",
        //     path: "/blog/",
        //     // subentries: SubEntries::MdDir("blog"), Don't be crazy? This
        //     // is just the menu, btw. See `struct Blog` below for the
        //     // 'real thing'. XX
        //     subentries: SubEntries::Static(&[]),
        // },
    ])),
    (Lang::De, Nav(&[
        NavEntry {
            name: "Willkommen",
            path: "/de.html",
            subentries: SubEntries::Static(&[]),
        },
        NavEntry {
            name: "Klima & Umwelt",
            path: "/umwelt.html",
            subentries: SubEntries::Static(&[]),
        },
        NavEntry {
            name: "Projekte",
            path: "/projekte.html",
            subentries: SubEntries::Static(&[]),
        },
        NavEntry {
            name: "Kontakt",
            path: "/kontakt.html",
            subentries: SubEntries::Static(&[]),
        },
        // NavEntry {
        //     name: "Blog",
        //     path: "/blog/",
        //     // subentries: SubEntries::MdDir("blog"), Don't be crazy? This
        //     // is just the menu, btw. See `struct Blog` below for the
        //     // 'real thing'. XX
        //     subentries: SubEntries::Static(&[]),
        // },
    ])),
];

// -----------------------------------------------------------------------------
// Main

lazy_static!{
    static ref ALLOCPOOL: AllocatorPool =
        AllocatorPool::new(1000000, true); // XX config
}

struct Tlskeys {
    crt: Vec<u8>,
    key: Vec<u8>,
}

fn lang_from_path(path: &PPath<KString>) -> Option<Lang> {
    // funny, can just take the first segment
    let p0 = path.segments().get(0)?;
    let base = base_and_suffix(&**p0)?.0;
    AList(LANG_FROM_PATH).get(&base).map(|(l, _)| *l)
}

fn sibling_from_path(path: &PPath<KString>) -> Option<String> {
    // funny, can just take the first segment
    let p0 = path.segments().get(0)?;
    let base = base_and_suffix(&**p0)?.0;
    let (_, sibling) = AList(LANG_FROM_PATH).get(&base)?;
    Some(format!("{sibling}.html"))
}

fn main() -> Result<()> {
    DO_WARN_THREAD.store(true, std::sync::atomic::Ordering::SeqCst);

    let sessionid_hasher = {
        let sessionid_hasher_secret = xgetenv("SESSIONID_HASHER_SECRET")?;
        let mut h = Hasher::new();
        h.update(sessionid_hasher_secret.as_bytes());
        h
    };

    let in_datadir = Arc::new({
        let base = getenv_or("DATADIR", Some("data"))?;
        move |subpath: &str| -> String {
            format!("{base}/{subpath}")
        }
    });
    let wwwdir = getenv("WWWDIR")?;
    let domainfallbackdir = getenv("DOMAINFALLBACKDIR")?;
    let wellknowndir: String = getenv("WELLKNOWNDIR")?.ok_or_else(
        || anyhow!("Missing WELLKNOWNDIR env var, e.g. /var/www/html/.well-known/"))?;
    let tlskeysfilebase = getenv("TLSKEYSFILEBASE")?;
    let is_dev = getenv("IS_DEV")?.is_some();

    let tlskeys = tlskeysfilebase.map(
        |base| -> Result<_> {
            Ok(Tlskeys {
                crt: my_read_to_string(format!("{base}.crt"))?.into_bytes(),
                key: my_read_to_string(format!("{base}.key"))?.into_bytes()
            })
        }).transpose()?;
    
    let footnotestyle: Arc<dyn StylingInterface> =
        match getenv_or("STYLE", Some("blog"))?.as_str() {
            "blog" => Arc::new(BlogStyle {}),
            "wikipedia" => Arc::new(WikipediaStyle {}),
            _ => bail!("no match for STYLE env var value"),
        };

    let site_owner = "Christian Jaeger";
    let style = Arc::new(WebsiteLayout {
        site_name: site_owner,
        copyright_owner: site_owner,
        nav: &NAV,
        header_contents: Box::new({
            let in_datadir = in_datadir.clone();
            move |html: &HtmlAllocator| -> Result<Flat<Node>> {
                Ok(Flat::One(
                    html.a([att("href", "/")], // i18n: just redirect again, OK?
                           [static_img(html,
                                       &in_datadir("static/headerbg2.jpg"),
                                       "/static/headerbg2.jpg",
                                       "",
                                       Some("headerpic"))?])?))
            }}),
        sibling_from_path: Box::new(sibling_from_path),
    });
    let preview_groupid = access_control_transaction(false, |trans| -> Result<_> {
        Ok(trans.xget_group_by_groupname("preview")?.id.expect("present from db"))
    })?;
    let mut router : MultiRouter<Arc<dyn Handler<Lang>>> = MultiRouter::new();
    router
        .add("/login", login_handler(style.clone()))
        .add("/bench", Arc::new(ExactFnHandler::new(website_benchmark::benchmark)))
        .add("/", markdownpage_handler(&in_datadir("de.de-en.md"), style.clone())) // XXX redir
    // --------------------------------------------
    // XX horrible hack, make a dir lister; also redirector for dir; and for non-language urls (ok that one will be above)
        .add("/en.html", markdownpage_handler(&in_datadir("en.en-de.md"), style.clone()))
        .add("/climate.html", markdownpage_handler(&in_datadir("climate.en-umwelt.md"), style.clone()))
        .add("/projects.html", markdownpage_handler(&in_datadir("projects.en-projekte.md"), style.clone()))
        .add("/contact.html", markdownpage_handler(&in_datadir("contact.en-kontakt.md"), style.clone()))

        .add("/de.html", markdownpage_handler(&in_datadir("de.de-en.md"), style.clone()))
        .add("/umwelt.html", markdownpage_handler(&in_datadir("umwelt.de-climate.md"), style.clone()))
        .add("/projekte.html", markdownpage_handler(&in_datadir("projekte.de-projects.md"), style.clone()))
        .add("/kontakt.html", markdownpage_handler(&in_datadir("kontakt.de-contact.md"), style.clone()))
    // --------------------------------------------
        .add("/static", Arc::new(FileHandler::new(in_datadir("static"))))
        .add("/blog", blog_handler(
            Blog::open(in_datadir("blog"), &ALLOCPOOL, footnotestyle.clone())?,
            style.clone()))
        .add("/preview", blog_handler(
            Blog::open(in_datadir("preview"), &ALLOCPOOL, footnotestyle)?,
            style.clone())
             .restricted_to_group(preview_groupid, style.clone()))
        .add("/p", markdowndir_handler(&in_datadir("p"), style.clone()))
        ;
    if let Some(wwwdir) = wwwdir {
        router.add("/", Arc::new(FileHandler::new(wwwdir)));
    }
    let router = Arc::new(router);
    let fallbackhandler = Arc::new(FileHandler::new(in_datadir("fallback")));

    let logbasedir = log_basedir()?;
    eprintln!("Logging to dir {logbasedir:?}");

    let new_hostsrouter = |is_https| -> Result<_> {
        let main_hostrouter = Arc::new(HostRouter {
            router: Some(router.clone()),
            fallback: Some(fallbackhandler.clone()),
            logs: Logs::open_in_basedir(
                &format!("{logbasedir}/christianjaeger.ch"), is_https)?
        });
        let mut hostsrouter =
            if is_dev {
                HostsRouter::new(
                    // domain fallback:
                    Some(main_hostrouter.clone()),
                    Logs::open_in_basedir(&logbasedir, is_https)?)
            } else {
                let domain_fallback_hostrouter =
                    if let Some(domain_fallback_dir) = &domainfallbackdir {
                        Some(Arc::new(HostRouter {
                            router: None,
                            fallback: Some(Arc::new(FileHandler::new(domain_fallback_dir))),
                            logs: Logs::open_in_basedir(
                                &format!("{logbasedir}/domain_fallback"), is_https)?
                        }))
                    } else {
                        warn!("missing DOMAINFALLBACKDIR env var, will return 404 instead");
                        None
                    };
                HostsRouter::new(
                    // domain fallback:
                    domain_fallback_hostrouter,
                    Logs::open_in_basedir(&logbasedir, is_https)?)
            };
        hostsrouter.add(
            "christianjaeger.ch",
            main_hostrouter);
        {
            // Must *not* redirect files for letsencrypt, thus need a
            // router for these:
            let mut letsencrypt_router : MultiRouter<Arc<dyn Handler<Lang>>>
                = MultiRouter::new();
            letsencrypt_router
                .add("/.well-known", Arc::new(FileHandler::new(&wellknowndir)));
            hostsrouter.add(
                "www.christianjaeger.ch",
                Arc::new(HostRouter {
                    router: Some(Arc::new(letsencrypt_router)),
                    fallback: Some(Arc::new(RedirectHandler::new(
                        move |request: &ARequest<Lang>| {
                            // XX this should be done in a better way.
                            let qs = request.query_string();
                            let qs_ =
                                if qs.is_empty()  {
                                    String::from("")
                                } else {
                                    format!("?{}", qs)
                                };
                            let s = if is_https { "s" } else { "" };
                            // is path_str *guaranteed* to start with a slash?
                            format!("http{s}://christianjaeger.ch{}{}",
                                    request.path_str(),
                                    qs_)
                        },
                        HttpResponseStatusCode::PermanentRedirect308
                    ))),
                    logs: Logs::open_in_basedir(
                        &format!("{logbasedir}/www.christianjaeger.ch"), is_https)?
                }));
        }
        Ok(Arc::new(hostsrouter))
    };

    // This was an attempt to have Rouille use a thread pool with
    // fixed size, allocating the max pool size ever wanted (enough
    // for handling attacks as well as the server can). Turns out the
    // tiny_http backend actually has its own pool so this is
    // completely pointless. `pool_size_and_stack_size` is from a
    // patch to Rouille from me.
    // let httpthreadpool_size = 1200; // per service
    // let httpstack_size = 8_000_000; // B, default 8 MiB?
    macro_rules! run {
        { $server_result:expr } => {
            $server_result.expect("it failed (I wanted `?`, why size business pls?)")
                // .pool_size_and_stack_size(httpthreadpool_size, httpstack_size)
                .run();
        }
    }

    // The worker thread pool is kept separate and much smaller, since
    // it keeps thread local state, also want CPU intensive part to
    // finish quickly.
    let workerthreadpool_size = 8 * thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let workerthreadpool = {
        let cfg = scoped_thread_pool::ThreadConfig::new()
            .prefix("scoped_website_worker");
        Arc::new(scoped_thread_pool::Pool::with_thread_config(
            workerthreadpool_size, cfg))
    };
    let http_thread = thread::Builder::new().name("website_http".into()).spawn({
        let addr = std::env::var("LISTEN_HTTP").unwrap_or("127.0.0.1:3000".into());
        let hostsrouter = new_hostsrouter(false)?;
        let workerthreadpool = workerthreadpool.clone();
        let sessionid_hasher = sessionid_hasher.clone();
        move || {
            run!(Server::new(
                addr.clone(),
                server_handler(
                    addr,
                    hostsrouter,
                    &ALLOCPOOL,
                    workerthreadpool,
                    sessionid_hasher,
                    lang_from_path,
                )));
        }
    })?;

    let https_thread = thread::Builder::new().name("website_https".into()).spawn({
        let addr = std::env::var("LISTEN_HTTPS").unwrap_or("127.0.0.1:3001".into());
        let hostsrouter = new_hostsrouter(true)?;
        let workerthreadpool = workerthreadpool.clone();
        let sessionid_hasher = sessionid_hasher.clone();
        move || {
            if let Some(tlskeys) = tlskeys {
                run!(Server::new_ssl(
                    addr.clone(),
                    server_handler(
                        addr,
                        hostsrouter,
                        &ALLOCPOOL,
                        workerthreadpool,
                        sessionid_hasher,
                        lang_from_path,
                    ),
                    tlskeys.crt,
                    tlskeys.key));
            } else {
                if is_dev {
                    // run fake service
                    run!(Server::new(
                        addr.clone(),
                        server_handler(
                            addr,
                            hostsrouter,
                            &ALLOCPOOL,
                            workerthreadpool,
                            sessionid_hasher,
                            lang_from_path,
                        )));
                } else {
                    warn!("don't have keys, thus not running the HTTPS service!");
                }
            }
        }
    })?;

    http_thread.join().expect("http thread should not panic");
    https_thread.join().expect("https thread should not panic");
    bail!("Server stopped.");
}

