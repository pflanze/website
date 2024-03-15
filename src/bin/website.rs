use std::sync::Arc;
use blake3::Hasher;
use kstring::KString;
use website::access_control::db::access_control_transaction;
use website::access_control::statements_and_methods::DO_WARN_THREAD;
use website::access_control::transaction::TransactError;
use website::access_control::types::GroupId;
use website::alist::AList;
use website::apachelog::Logs;
use website::acontext::AContext;
use website::blog::Blog;
use website::ahtml::{AllocatorPool, Flat, HtmlAllocator, Node, att, AHTML_TRACE};
use anyhow::{Result, bail, anyhow};
use website::hostrouter::{HostRouter, HostsRouter};
use website::http_response_status_codes::HttpResponseStatusCode;
use website::imageinfo::static_img;
use website::io_util::my_read_to_string;
use website::lang_en_de::Lang;
use website::path::base_and_suffix;
use website::ppath::PPath;
use website::rouille_runner::{RouilleRunner, Tlskeys};
use website::style::footnotes::{WikipediaStyle, BlogStyle};
use website::handler::{ExactFnHandler, RedirectHandler};
use website::handler::FileHandler;
use lazy_static::lazy_static;
use website::markdown::StylingInterface;
use website::nav::{Nav, NavEntry, SubEntries};
use website::router::MultiRouter;
use website::util::{log_basedir, getenv_or, getenv, xgetenv, getenv_bool};
use website::webparts::{markdownpage_handler, blog_handler,
                        login_handler, Restricted, unlisted_markdowndir_handler,
                        language_handler, mixed_dir_handler};
use website::website_layout::WebsiteLayout;
use website::handler::Handler;
use website::{website_benchmark, warn};


// HACK for now
const LANG_FROM_PATH: &[(&str, (Lang, &str))] = &[
    // (basename, (is lang, has sibling))
    ("en", (Lang::En, "de")),
    ("climate", (Lang::En, "umwelt")),
    ("contact", (Lang::En, "kontakt")),
    ("projects", (Lang::En, "projekte")),
    ("about", (Lang::En, "person")),
    // --
    ("de", (Lang::De, "en")),
    ("umwelt", (Lang::De, "climate")),
    ("projekte", (Lang::De, "projects")),
    ("kontakt", (Lang::De, "contact")),
    ("person", (Lang::De, "about")),
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
            name: "About me",
            path: "/about.html",
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
            name: "Über mich",
            path: "/person.html",
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

fn get_group_id(group_name: &str) -> Result<GroupId, TransactError<anyhow::Error>> {
    access_control_transaction(false, |trans| -> Result<_> {
        Ok(trans.xget_group_by_groupname(group_name)?.id.expect("present from db"))
    })
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
    let is_dev = getenv_bool("IS_DEV")?;
    let ahtml_trace = getenv_bool("AHTML_TRACE")?;
    dbg!(ahtml_trace);

    AHTML_TRACE.store(ahtml_trace, std::sync::atomic::Ordering::Relaxed);

    let tlskeys = tlskeysfilebase.map(
        |base| -> Result<_> {
            Ok(Tlskeys {
                crt: my_read_to_string(format!("{base}.crt"))?.into_bytes(),
                key: my_read_to_string(format!("{base}.key"))?.into_bytes()
            })
        }).transpose()?;
    
    let footnotestyle = {
        let s : Arc<dyn StylingInterface> =
            match getenv_or("STYLE", Some("blog"))?.as_str() {
                "blog" => Arc::new(BlogStyle {}),
                "wikipedia" => Arc::new(WikipediaStyle {}),
                _ => bail!("no match for STYLE env var value"),
            };
        move || s.clone()
    };

    let site_owner = "Christian Jaeger";
    let style = {
        let s = Arc::new(WebsiteLayout {
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
        move || s.clone()
    };
    let preview_groupid = get_group_id("preview")?;
    let fellowship_groupid = get_group_id("fellowship")?;
    let router = {
        let mut router : MultiRouter<Arc<dyn Handler<Lang>>> = MultiRouter::new();
        router
            .add("/login", login_handler(style()))
            .add("/bench", Arc::new(ExactFnHandler::new(website_benchmark::benchmark)))
            .add("/", language_handler())
        // --------------------------------------------
        // XX hack for dual language; todo: make a multi-lingual dir
        // lister (for single-language purposes, `mixed_dir_handler`
        // already exists)
            .add("/en.html", markdownpage_handler(&in_datadir("en.en-de.md"), style()))
            .add("/climate.html", markdownpage_handler(&in_datadir("climate.en-umwelt.md"), style()))
            .add("/projects.html", markdownpage_handler(&in_datadir("projects.en-projekte.md"), style()))
            .add("/about.html", markdownpage_handler(&in_datadir("about.en-person.md"), style()))
            .add("/contact.html", markdownpage_handler(&in_datadir("contact.en-kontakt.md"), style()))

            .add("/de.html", markdownpage_handler(&in_datadir("de.de-en.md"), style()))
            .add("/umwelt.html", markdownpage_handler(&in_datadir("umwelt.de-climate.md"), style()))
            .add("/projekte.html", markdownpage_handler(&in_datadir("projekte.de-projects.md"), style()))
            .add("/person.html", markdownpage_handler(&in_datadir("person.de-about.md"), style()))
            .add("/kontakt.html", markdownpage_handler(&in_datadir("kontakt.de-contact.md"), style()))
        // --------------------------------------------
            .add("/static", Arc::new(FileHandler::new(in_datadir("static"))))
            .add("/blog", blog_handler(
                Blog::open(in_datadir("blog"), &ALLOCPOOL, footnotestyle())?,
                style()))
            .add("/preview", blog_handler(
                Blog::open(in_datadir("preview"), &ALLOCPOOL, footnotestyle())?,
                style())
                 .restricted_to_group(preview_groupid, style()))
            .add("/fellowship", mixed_dir_handler("www-data/fellowship", style())
                .restricted_to_group(fellowship_groupid, style()))
            .add("/p", unlisted_markdowndir_handler(&in_datadir("p"), style()))
            ;
        if let Some(wwwdir) = wwwdir {
            router.add("/", Arc::new(FileHandler::new(wwwdir)));
        }
        let r = Arc::new(router);
        move || r.clone()
    };
    let fallbackhandler = Arc::new(FileHandler::new(in_datadir("fallback")));

    let logbasedir = log_basedir()?;
    eprintln!("Logging to dir {logbasedir:?}");

    let new_hostsrouter = |is_https| -> Result<_> {
        let main_hostrouter = Arc::new(HostRouter {
            router: Some(router()),
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
                        move |context: &AContext<Lang>| {
                            // XX this should be done in a better way.
                            let qs = context.query_string();
                            let qs_ =
                                if qs.is_empty()  {
                                    String::from("")
                                } else {
                                    format!("?{}", qs)
                                };
                            let s = if is_https { "s" } else { "" };
                            // is path_str *guaranteed* to start with a slash?
                            format!("http{s}://christianjaeger.ch{}{}",
                                    context.path_str(),
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

    let rouille_runner = RouilleRunner::new(
        &ALLOCPOOL,
        sessionid_hasher,
        Arc::new(lang_from_path));

    let http_thread = {
        let addr = std::env::var("LISTEN_HTTP").unwrap_or("127.0.0.1:3000".into());
        let hostsrouter = new_hostsrouter(false)?;
        rouille_runner.run_server(
            "website_http",
            addr,
            None,
            hostsrouter)?
    };

    let https_thread = {
        let addr = std::env::var("LISTEN_HTTPS").unwrap_or("127.0.0.1:3001".into());
        let hostsrouter = new_hostsrouter(true)?;
        if let Some(tlskeys) = tlskeys {
            Some(rouille_runner.run_server(
                "website_https",
                addr,
                Some(tlskeys),
                hostsrouter)?)
        } else {
            if is_dev {
                // run fake service
                Some(rouille_runner.run_server(
                    "website_https",
                    addr,
                    None,
                    hostsrouter)?)
            } else {
                warn!("don't have keys, thus not running the HTTPS service!");
                None
            }
        }
    };

    http_thread.join().expect("http thread should not panic");
    https_thread.map(|t| t.join().expect("https thread should not panic"));
    bail!("Server stopped.");
}

