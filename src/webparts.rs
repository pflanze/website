
//! Components making up a website, parameterized via a trait.

use std::{path::{PathBuf, Path},
          sync::{Arc, Mutex},
          time::{SystemTime, Instant, Duration},
          fmt::Debug};

use anyhow::{Result, Context, anyhow, bail};
use blake3::Hasher;
use chrono::NaiveDate;
use kstring::KString;
use rand::{prelude::thread_rng, Rng};
use rand_distr::Weibull;
use rouille::{Response, Request, post_input, session::session};
use scoped_thread_pool::Pool;

use crate::{acontext::AContext,
            ahtml::{HtmlAllocator, AId, Node, P_META, TryCollectBody, AllocatorPool,
                    att, opt_att},
            webutils::{htmlresponse, request_resolve_relative, errorpage_from_status},
            http_response_status_codes::HttpResponseStatusCode,
            markdown::MarkdownFile,
            handler::{Handler, ExactFnHandler, FnHandler, FileHandler},
            blog::{Blog, BlogNode, BlogPostIndex},
            ppath::PPath,
            trie::TrieIterReportStyle,
            apachelog::{Logs, log_combined},
            hostrouter::HostsRouter,
            http_request_method::{HttpRequestMethodGrouped, HttpRequestMethodSimple},
            access_control::{check_username_password, CheckAccessErrorKind,
                             db::access_control_transaction, types::{SessionData, GroupId}, statements_and_methods::sessionid_hash},
            in_threadpool::in_threadpool,
            aresponse::{AResponse, ToAResponse},
            time_util::{self, now_unixtime},
            ipaddr_util::IpAddrOctets,
            auri::{AUriLocal, QueryString},
            notime, path::{path_append, extension_eq, base, suffix}, language::Language};
use crate::{try_result, warn, nodt, time_guard};

// ------------------------------------------------------------------
// The mid-level parts

/// Make a handler for Rouille's `start_server` procedure.
pub fn server_handler<'t, L: Language + Default>(
    listen_addr: String,
    hostsrouter: Arc<HostsRouter<L>>,
    allocatorpool: &'static AllocatorPool,
    threadpool: Arc<Pool>,
    sessionid_hasher: Hasher,
    lang_from_path: impl Fn(&PPath<KString>) -> Option<L> + Send + Sync,
) -> impl for<'r> Fn(&'r Request) -> Response {
    move |request: &Request| -> Response {
        time_guard!("server_handler"); // timings including infrastructure cost
        session(request, "sid", 3600 /*sec*/, |session| {
            let aresponse = in_threadpool(threadpool.clone(), || -> AResponse {
                let okhandler = |context: &AContext<L>| -> AResponse {
                    log_combined(
                        context,
                        || -> (Arc<Mutex<Logs>>, anyhow::Result<AResponse>) {
                            let method = context.method();
                            let unimplemented = |methodname| {
                                warn!("method {methodname:?} not implemented (yet)");
                                (hostsrouter.logs.clone(),
                                 Ok(errorpage_from_status(
                                     HttpResponseStatusCode::NotImplemented501).into()))
                            };
                            match method.to_grouped() {
                                HttpRequestMethodGrouped::Simple(simplemethod) => {
                                    let mut guard = allocatorpool.get();
                                    let allocator = guard.allocator();
                                    if let Some(host) = context.host() {
                                        let lchost = host.to_lowercase();
                                        if let Some(hostrouter) = hostsrouter.routers.get(
                                            &KString::from_string(lchost))
                                        {
                                            return hostrouter.handle_request(
                                                context, simplemethod, allocator)
                                        }
                                    }
                                    if let Some(fallback) = &hostsrouter.fallback {
                                        return fallback.handle_request(
                                            context, simplemethod, allocator)
                                    }
                                }
                                HttpRequestMethodGrouped::Document(documentmethod) => {
                                    return unimplemented(
                                        documentmethod.to_http_request_method().as_str())
                                }
                                HttpRequestMethodGrouped::Special(specialmethod) =>
                                    // XX should at least implement OPTIONS, or ?
                                    return unimplemented(
                                        specialmethod.to_http_request_method().as_str())
                                    // match specialmethod {
                                    //     HttpRequestMethodSpecial::OPTIONS =>
                                    //         return unimplemented(),
                                    //     HttpRequestMethodSpecial::TRACE =>
                                    //         return unimplemented(),
                                    //     HttpRequestMethodSpecial::CONNECT =>
                                    //         return unimplemented(),
                                    // },
                            }
                            (hostsrouter.logs.clone(),
                             Ok(errorpage_from_status(HttpResponseStatusCode::NotFound404)
                                .into()))
                        })
                };
                match AContext::new(request, &listen_addr, session, &sessionid_hasher,
                                    &lang_from_path) {
                    Ok(mut context) => {
                        let mut aresponse= okhandler(&context);
                        context.set_headers(&mut aresponse.response.headers);
                        aresponse
                    }
                    Err(e) => {
                        warn!("{e}");
                        errorpage_from_status(
                            HttpResponseStatusCode::InternalServerError500).into()
                    }
                }
            }).expect("only ever fails if thread fails outside catch_unwind");
            let AResponse { response, sleep_until } = aresponse;
            if let Some(t) = sleep_until {
                time_util::sleep_until(t);
            }
            response
        })
    }
}

// ------------------------------------------------------------------
// The mid-level parts, building elements

pub fn pair<'a>(html: &'a HtmlAllocator) -> impl Fn(AId<Node>, AId<Node>) -> Result<AId<Node>> + 'a
{
    move |a, b| {
        html.div([att("class", "pair")],
                 [
                     html.div([att("class", "pair_a")],
                              [a])?,
                     html.div([att("class", "pair_b")],
                              [b])?,
                 ])
    }
}

// pub fn single<'a>(html: &'a Allocator) -> impl Fn(AId<Node>) -> Result<AId<Node>> + 'a
// {
//     move |a| {
//         html.div([att("class", "single")],
//                  [a])
//     }
// }

pub fn buttonrow<'a, const N: usize>(
    html: &'a HtmlAllocator
) -> impl Fn([AId<Node>; N]) -> Result<AId<Node>> + 'a
{
    move |buttons| {
        html.div([att("class", "buttonrow")],
                 buttons)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PopupBoxKind {
    Dialog,
    Error(HttpResponseStatusCode),
    // Informational,
}

pub fn popup_box<'a>(
    html: &'a HtmlAllocator
) -> impl Fn(PopupBoxKind, AId<Node>, AId<Node>) -> Result<AId<Node>> + 'a
{
    move |kind, title, body| {
        let box_style = match kind {
            PopupBoxKind::Dialog => "dialog_box",
            PopupBoxKind::Error(_) => "error_box",
        };
        html.div([att("class", "dialog_box_container")],
                 [
                     html.div([att("class", box_style)],
                              [
                                  html.div([att("class", "dialog_box_title")],
                                           [title])?,
                                  html.div([att("class", "dialog_box_body")],
                                           [body])?
                              ])?
                 ])
    }
}

pub fn show_popup_box_page<L: Language>(
    context: &AContext<L>,
    html: &HtmlAllocator,
    style: &Arc<dyn LayoutInterface<L>>,
    box_kind: PopupBoxKind,
    box_title: AId<Node>,
    box_body: AId<Node>,
) -> Result<Option<Response>>
{
    let popup_box = popup_box(html);
    let status = match box_kind {
        PopupBoxKind::Dialog => HttpResponseStatusCode::OK200,
        PopupBoxKind::Error(status) => status
    };
    Ok(Some(htmlresponse(html, status, |html| {
        style.page(
            context,
            html,
            None,
            None,
            None,
            None,
            None,
            popup_box(
                box_kind,
                box_title,
                box_body)?,
            None,
            None)
        })?))
}

// ------------------------------------------------------------------
// The higher-level parts, building blocks

pub trait LayoutInterface<L: Language>: Send + Sync {
    /// Build a whole HTML page from the given parts
    fn page(
        &self,
        context: &AContext<L>,
        html: &HtmlAllocator,
        // Can't be preserialized HTML, must be string node. If
        // missing, a default title should be used (usually the site
        // name that would be appended or prepended to the title):
        head_title: Option<AId<Node>>,
        // Used inside the body. Same contents as head_title, but may
        // be preserialized HTML; must not contain wrapper element
        // like <h1>:
        title: Option<AId<Node>>,
        breadcrumb: Option<AId<Node>>,
        toc: Option<AId<Node>>,
        lead: Option<AId<Node>>,
        main: AId<Node>,
        footnotes: Option<AId<Node>>,
        last_modified: Option<SystemTime>,
    ) -> Result<AId<Node>>;

    fn blog_index_title(
        &self,
        subpath_segments: Option<&[KString]> // path segments if below main page
    ) -> String;
}

/// This re-parses the markdown on every request.
fn markdownprocessor<L: Language>(
    style: Arc<dyn LayoutInterface<L>>,
    context: &AContext<L>,
    path: PathBuf,
    html: &HtmlAllocator    
) -> Result<Response>
{
    htmlresponse(html, HttpResponseStatusCode::OK200, |html| {
        let stat = path.metadata().with_context(
            || anyhow!("stat on {:?}", path.to_string_lossy()))?;
        let mdfile = MarkdownFile::new(path);
        let pmd = mdfile.process_to_html(html)?;
        let title =
            if let Some(body) = pmd.meta().title() {
                // body can contain <P> if it's a sep para within <title>, so unwrap it
                Some(html.span([], body.unwrap_elements(*P_META, false, html)?)?)
            } else {
                None
            };
        // XX process footnotes!
        style.page(
            context,
            html,
            // html.kstring(mdmeta.title_string(html, "(missing title)")?)?,
            title,
            title,
            None, // breadcrumb
            None, // XX just turn off globally  Some(pmd.meta().toc_html_fragment(html)?),
            None, // lead XX?
            pmd.fixed_html(html)?,
            None, // XX
            Some(stat.modified()?)
        )
    })
}

// To place a particular md file via its fspath.
pub fn markdownpage_handler<L: Language + 'static>(
    file_path: &str,
    style: Arc<dyn LayoutInterface<L>>
) -> Arc<dyn Handler<L>>
{
    let path = PathBuf::from(file_path);
    Arc::new(ExactFnHandler::new(
        move |
        context: &AContext<L>, method: HttpRequestMethodSimple, html: &HtmlAllocator
            | -> Result<AResponse>
        {
            if method.is_post() {
                bail!("can't POST to a markdownpage"); // currently, anyway
            }
            markdownprocessor(style.clone(), context, path.clone(), html)
                .map(AResponse::from)
        }
    ))
}

#[derive(Debug, Clone, Copy)]
pub enum DirIndexMode {
    /// No directory indexing, if a path to a directory is given, the
    /// handler will decline.
    None,
    /// If the path goes to a directory, look for an `index.md` file,
    /// if present, redirect to add a trailing `/` unless already
    /// present, then serve as markdown.
    IndexMd,
    // Future?: `Dirlisting`, show a directory listing. Or provide
    // that in a different type?
}

/// Generate a markdown page from a file with suffix `.md` if
/// `path_rest` ends with suffix `.html`, or if `indexing_mode` is
/// `IndexMd` and `path_rest` goes to a directory then looks for an
/// `index.md` file and serves that "after" doing a redirect to add a
/// `/`, or otherwise decline via `Ok(None)`.
// Mess. Probably did some other versions with similar code, todo:
// proper factoring.
fn generate_markdown_page<L: Language + 'static>(
    base_path: &Path,
    path_rest: &PPath<KString>,
    indexing_mode: DirIndexMode,
    context: &AContext<L>,
    style: Arc<dyn LayoutInterface<L>>,
    html: &HtmlAllocator,
) -> Result<Option<Response>> {
    let suffix =
        if let Some(last) = path_rest.segments().last() {
            suffix(last)
        } else {
            None
        };
    let has_html_suffix = suffix.and_then(|s| Some(s == "html")).unwrap_or(false);
    let path_rest_string = path_rest.to_string();
    if has_html_suffix {
        let mut fspath = path_append(base_path, &base(&path_rest_string).expect(
            "succeeds because we know it has a html suffix from above"));
        if ! fspath.set_extension("md") {
            bail!("missing file name? not possible?")
        }
        // warn!("have fspath = {fspath:?}");
        try_result!{
            let not_found = || {
                // XX todo: return styled 404, not generic error page
                Ok(Some(errorpage_from_status(HttpResponseStatusCode::NotFound404)))
            };
            match fspath.metadata() {
                Ok(stat) =>
                    if stat.is_file() {
                        Ok(Some(markdownprocessor(style, context, fspath, html)?))
                    } else {
                        warn!("found {fspath:?} but it's not a file, thus report 404");
                        not_found()
                    }
                Err(e) => match e.kind() {
                    std::io::ErrorKind::NotFound => return not_found(),
                    _ => {
                        warn!("Error getting metadata: {e:?} \
                               for path {:?}", &fspath);
                        // but return 404 anyway, ok?
                        not_found()
                    }
                }
            }
        }
    } else {
        // no html suffix
        match indexing_mode {
            DirIndexMode::None => Ok(None),
            DirIndexMode::IndexMd => {
                let mut fspath = path_append(base_path, &path_rest_string);
                match fspath.metadata() {
                    Ok(stat) =>
                        if stat.is_file() {
                            // path is not .html nor directory, so we won't serve it
                            Ok(None)
                        } else if stat.is_dir() {
                            fspath.push("index.md");
                            match fspath.metadata() {
                                Ok(stat_index_md) =>
                                    if stat_index_md.is_file() {
                                        if path_rest.ends_with_slash() {
                                            Ok(Some(markdownprocessor(
                                                style, context, fspath, html)?))
                                        } else {
                                            Ok(Some(
                                                context.redirect_302_with_query(
                                                    &context.path().as_dir())))
                                        }
                                    } else {
                                        warn!("found item {:?} but it's not a file, ignoring",
                                              &fspath);
                                        Ok(None)
                                    },
                                Err(e) => {
                                    warn!("Error getting metadata: {e:?} \
                                           for path {:?}", &fspath);
                                    Ok(None)
                                }
                            }
                        } else {
                            warn!("not a file nor dir: {:?}", &fspath);
                            Ok(None)
                        }
                    Err(e) => {
                        warn!("Error getting metadata: {e:?} \
                               for path {:?}", &fspath);
                        Ok(None)
                    }
                }
            }
        }
    }
}

/// Serve markdown files from sub-paths from the given `dir_path`;
/// sub-paths can contain directory segments. Only requests with path
/// suffix `.html` are being served (otherwise the handler declines
/// handling), and a file with suffix `.md` in it's place is read if
/// available (otherwise a 404 result is returned).

/// There is no directory listing, and delivery is delayed by around
/// 0.2 seconds to hide potential side channels that would enable path
/// discovery as well as to make brute forcing harder. This handler is
/// thus suitable to host unlisted files only meant to be reachable via
/// an explicitly shared URL. You still need to choose sufficiently
/// random sub-paths for them to evade brute forcing!
pub fn markdowndir_handler<L: Language + 'static>(
    dir_path: &str,
    style: Arc<dyn LayoutInterface<L>>
) -> Arc<dyn Handler<L>>
{
    let base_path = PathBuf::from(dir_path);
    Arc::new(FnHandler::new(
        move |
        context: &AContext<L>,
        method: HttpRequestMethodSimple,
        path_rest: &PPath<KString>,
        html: &HtmlAllocator
            | -> Result<Option<AResponse>>
        {
            if let Some(last) = path_rest.segments().last() {
                if ! extension_eq(&last, "html") {
                    return Ok(None);
                }
            } else {
                // no segment, no suffix, no serving.
                return Ok(None);
            }
            if method.is_post() {
                bail!("can't POST to a markdownpage"); // currently, anyway
            }
            if ! path_rest.is_canonical() {
                bail!("requested path rest isn't canonical: {:?}",
                      path_rest.to_string())
            }
            
            // COPY-PASTE from login_handler, except using a shorter delay
            let start: Instant = Instant::now();
            let delayed = |response: Result<Option<Response>>| -> Result<Option<AResponse>>
            {
                let _micros: Weibull<f64> = Weibull::new(200000., 20.)?;
                let micros: f64 = thread_rng().sample(_micros);
                let target = start.checked_add(Duration::from_micros(micros as u64))
                    .expect("does not fail (overflow) because we only add a second");
                response.map(|v| v.map(|r| r.to_aresponse(Some(target))))
            };
            // /COPY-PASTE
            delayed(generate_markdown_page(&base_path,
                                           path_rest,
                                           DirIndexMode::None,
                                           context,
                                           style.clone(),
                                           html))
        }
    ))
}

/// Serve markdown and static files from sub-paths from the given
/// `dir_path`; sub-paths can contain directory segments. Requests
/// with path suffix `.html` are handled by looking for a file with
/// the suffix `md` in its place, other files are served as static
/// files.

/// There is no directory listing (but it does try to serve `index.md`
/// for directories), but also no delivery delay, thus this handler is
/// *not* suitable for serving unlisted files!
pub fn mixed_dir_handler<L: Language + 'static>(
    dir_path: &str,
    style: Arc<dyn LayoutInterface<L>>
) -> Arc<dyn Handler<L>>
{
    let base_path = PathBuf::from(dir_path);
    let file_handler = FileHandler::new(&base_path);
    Arc::new(FnHandler::new(
        move |
        context: &AContext<L>,
        method: HttpRequestMethodSimple,
        path_rest: &PPath<KString>,
        html: &HtmlAllocator
            | -> Result<Option<AResponse>>
        {
            if method.is_post() {
                bail!("can't POST to a mixed dir"); // currently, anyway
            }
            if ! path_rest.is_canonical() {
                bail!("requested path rest isn't canonical: {:?}",
                      path_rest.to_string())
            }
            let optresponse = generate_markdown_page(&base_path,
                                                     path_rest,
                                                     DirIndexMode::IndexMd,
                                                     context,
                                                     style.clone(),
                                                     html)?;
            Ok(match optresponse {
                Some(response) => Some(response.into()),
                None => file_handler.call(context, method, path_rest, html)?
            })
        }))
}


fn format_naivedate(nd: NaiveDate) -> String {
    format!("{}", nd)
}

pub fn blog_handler<L: Language + 'static>(
    blog: Arc<Blog>, style: Arc<dyn LayoutInterface<L>>
) -> Arc<dyn Handler<L>>
{
    // dbg!(&blog.blogcache());
    Arc::new(FnHandler::new(
        move |
        context: &AContext<L>,
        method: HttpRequestMethodSimple,
        path: &PPath<KString>,
        html: &HtmlAllocator
            | -> Result<Option<AResponse>>
        {
            nodt!("blog", path);
            if method.is_post() {
                bail!("can't POST to blog"); // currently, anyway
            }
            let with_slash = context.path().ends_with_slash();
            let blogcache = blog.blogcache();
            if let Some(trie) = blogcache.router.get_trie(path) {
                let blognode = trie.endpoint().expect(
                    "every trie node in a blog trie has an endpoint");
                match blognode {
                    BlogNode::BlogPost(blogpost) => {
                        nodt!("blogpost", pathrest);
                        
                        // an individual post; XX check that the part of
                        // the path used contains the date?
                        let head_title = html.kstring(blogpost.title_plain.clone())?;
                        let title = html.preserialized(&blogpost.title_html)?;
                        let toc = html.preserialized(&blogpost.toc)?;
                        let lead = blogpost.lead.as_ref()
                            .map(|a| html.preserialized(a)).transpose()?;
                        let main = html.preserialized(&blogpost.main)?;
                        let opt_footnotes =
                            if blogpost.num_footnotes > 0 {
                                Some(html.preserialized(&blogpost.footnotes)?)
                            } else {
                                None
                            };
                        let breadcrumb =
                            html.preserialized(blogpost.breadcrumb.with_slash(
                                with_slash))?;
                        let resp =
                            htmlresponse(html, HttpResponseStatusCode::OK200, |html| {
                                Ok(style.page(
                                    context,
                                    html,
                                    Some(head_title),
                                    Some(title),
                                    Some(breadcrumb),
                                    Some(toc),
                                    lead,
                                    main,
                                    opt_footnotes,
                                    Some(blogpost.modified())
                                )?)
                            })?;
                        Ok(Some(resp.into()))
                    }
                    BlogNode::BlogPostIndex(BlogPostIndex { breadcrumb }) => {
                        nodt!("blog index");
                        let iter = trie.iter(true,
                                             TrieIterReportStyle::BeforeRecursing);
                        let resp =
                            htmlresponse(html, HttpResponseStatusCode::OK200, |html| {
                                let (archivetitle, breadcrumb) =
                                    if let Some(breadcrumb) = breadcrumb {
                                        (
                                            html.string(
                                                style.blog_index_title(Some(path.segments())))?,
                                            Some(html.preserialized(
                                                breadcrumb.with_slash(with_slash))?)
                                        )
                                    } else {
                                        (
                                            html.string(
                                                style.blog_index_title(None))?,
                                            None
                                        )
                                    };
                                style.page(
                                    context,
                                    html,
                                    Some(archivetitle),
                                    Some(archivetitle),
                                    breadcrumb,
                                    None, // toc
                                    None, // lead
                                    html.ul(
                                        [],
                                        iter.filter_map(
                                            |(path1, trie)| -> Option<Result<AId<Node>>> {
                                                let r: Result<Option<AId<Node>>> = try_result!{
                                                    let blognode =
                                                        trie.endpoint().expect(
                                                            "every trie node in a blog trie \
                                                             has an endpoint");
                                                    let blogpost =
                                                        match blognode {
                                                            BlogNode::BlogPost(p) => p,
                                                            BlogNode::BlogPostIndex(_) => {
                                                                return Ok(None)
                                                            }
                                                       };

                                                    let datestr =
                                                        format_naivedate(
                                                            blogpost.publish_date);
                                                    let url =
                                                        request_resolve_relative(
                                                            context,
                                                            PPath::new(false, false,
                                                                       path1));
                                                    Ok(Some(html.li(
                                                        [],
                                                        [
                                                            html.str(&datestr)?,
                                                            html.str(" - ")?,
                                                            html.a(
                                                                [att("href", &url)],
                                                                [
                                                                    html.preserialized(
                                                                        &blogpost.title_html)?
                                                                ])?
                                                        ])?))
                                                };
                                                r.transpose()
                                            }).try_collect_body(html)?)?,
                                    None,
                                    None)
                            })?;
                        Ok(Some(resp.into()))
                    }
                }
            } else {
                Ok(None)
            }
        }))
}

fn show_login_form<L: Language>(
    context: &AContext<L>,
    html: &HtmlAllocator,
    style: &Arc<dyn LayoutInterface<L>>,
    error: Option<String>,
    username: Option<String>,
    return_path: Option<String>,
) -> Result<Option<Response>>
{
    let pair = pair(html);
    let buttonrow = buttonrow(html);
    let form = html.form(
        [att("action", context.path_str()), att("method", "POST")],
        [
            if let Some(error) = error {
                html.div([att("class", "form_error")],
                         [html.string(error)?])?
            } else {
                html.empty_node()?
            },
            pair(html.str("Username:")?,
                 html.input([att("name", "username"), att("type", "text"),
                             opt_att("value", username)],
                            [])?)?,
            pair(html.str("Password:")?,
                 html.input([att("name", "password"), att("type", "password")],
                            [])?)?,
            if let Some(return_path) = return_path {
                html.input([att("name", "return_path"), att("type", "hidden"),
                            att("value", return_path)],
                           [])?
            } else {
                html.empty_node()?
            },
            buttonrow([
                html.button([att("type", "submit")],
                            [html.str("OK")?])?
            ])?,
        ])?;

    show_popup_box_page(context, html, style,
                        PopupBoxKind::Dialog,
                        html.string(format!("Login for {}",
                                            context.host_or_listen_addr()))?,
                        form)
}

pub fn login_handler<L: Language + 'static>(
    style: Arc<dyn LayoutInterface<L>>
) -> Arc<dyn Handler<L>> {
    Arc::new(FnHandler::new(
        move |
        context: &AContext<L>,
        method: HttpRequestMethodSimple,
        _path: &PPath<KString>,
        html: &HtmlAllocator
            | -> Result<Option<AResponse>>
        {
            let show_form = |
            error: Option<String>,
            username: Option<String>,
            return_path: Option<String>,
            | {
                show_login_form(context, html, &style, error, username, return_path)
            };

            let immediate = |response: Result<Option<Response>>| -> Result<Option<AResponse>>
            {
                response.map(|v| v.map(AResponse::from))
            };
            if method.is_post() {
                let inp = post_input!(context.request(), {
                    username: String,
                    password: String,
                    return_path: Option<String>
                })?;
                // Check rate limiting:
                // access_control_transaction(|trans| {
                //     // XX
                //     Ok(())
                // })?;
                

                // We are actually going to check the login:
                let start: Instant = Instant::now();
                let delayed = |response: Result<Option<Response>>| -> Result<Option<AResponse>>
                {
                    let _micros: Weibull<f64> = Weibull::new(1100000., 20.)?;
                    let micros: f64 = thread_rng().sample(_micros);
                    let target = start.checked_add(Duration::from_micros(micros as u64))
                        .expect("does not fail (overflow) because we only add a second");
                    response.map(|v| v.map(|r| r.to_aresponse(Some(target))))
                };
                match check_username_password(inp.username.trim(),
                                              &inp.password) {
                    Ok(Some(user)) => {
                        // Mark session as logged in
                        let user_id = user.id.expect("coming from db has an id");
                        let session_id = context.session_id();
                        let now_unixtime = now_unixtime();
                        let ip = context.client_ip().octets();
                        let hash = sessionid_hash(context.sessionid_hasher(), session_id);
                        access_control_transaction(true, |trans| -> Result<()> {
                            // Check if the session is already active
                            // (possible if data was stored before logging in)
                            if let Some(mut sessiondata) =
                                trans.get_sessiondata_by_sessionid_hash(&hash)?
                            {
                                if let Some(prev_user_id) = sessiondata.user_id {
                                    // Can happen if using back button
                                    // to get back to login form and
                                    // logging in again. Or not: if we
                                    // redirect right away in this
                                    // case -- XX
                                    if prev_user_id != user_id {
                                        // Not sure if this could happen.
                                        bail!("logged in concurrently as another user? \
                                               {prev_user_id:?} vs. {user_id:?}")
                                    }
                                    // Otherwise fine, do nothing except update timestamp
                                } else {
                                    sessiondata.user_id = Some(user_id);
                                    if let Some(oldip) = &sessiondata.ip {
                                        if *oldip != ip {
                                            warn!("login on same session again, previously \
                                                   from ip {oldip:?}, now {ip:?}");
                                        }
                                    }
                                    sessiondata.ip = Some(ip.clone());
                                }
                                sessiondata.last_request_time = now_unixtime;
                                trans.update_sessiondata(&sessiondata)?;
                            } else {
                                // create it
                                let sessiondata = SessionData::new(
                                    None,
                                    session_id,
                                    now_unixtime,
                                    Some(user_id),
                                    Some(ip.clone()),
                                    context.sessionid_hasher()
                                );
                                trans.insert_sessiondata(&sessiondata)?;
                            }
                            Ok(())
                        })?;
                        
                            
                        let target = inp.return_path.unwrap_or_else(|| "/".into());
                        // *Does* it have to sleep when succeeding? It
                        // does so that attackers cannot potentially
                        // interpret the result early.
                        delayed(
                            Ok(Some(Response::redirect_302(target))))
                    }
                    Ok(None) => {
                        delayed(
                            show_form(Some("Invalid username or password".into()),
                                      Some(inp.username),
                                      inp.return_path))
                    }
                    Err(e) => match &*e {
                        CheckAccessErrorKind::InputCheckFailure(e) => {
                            immediate(
                                show_form(Some(format!("{e}")),
                                          Some(inp.username),
                                          inp.return_path))
                        }
                        _ => Err(e)?
                    }
                }
            } else {
                let return_path = context.get_param("return_path");
                immediate(show_form(None, None, return_path))
            }
        }))
}


/// Tie via GroupId: requires that Ids are never re-used in the
/// database! XX double-check sqlite.
pub trait Restricted<L: Language> {
    fn restricted_to_group(
        self,
        group: GroupId,
        style: Arc<dyn LayoutInterface<L>>,
    ) -> Self;
}

enum LoginState {
    NeedLogin,
    NotAllowed,
    Allowed
}

impl<L: Language + 'static> Restricted<L> for Arc<dyn Handler<L>> {
    fn restricted_to_group(
        self,
        group_id: GroupId,
        style: Arc<dyn LayoutInterface<L>>,
    ) -> Self {
        Arc::new(FnHandler::new(move |context, method, path, html| -> Result<Option<AResponse>> {
            let session = context.session();
            // if ! session.client_has_sid() {
            //     todo!()
            // }
            let hash = sessionid_hash(context.sessionid_hasher(), session.id());
            let state = access_control_transaction(true, move |trans| -> Result<_> {
                if let Some(mut sessiondata) = notime!{
                    "get_sessiondata_by_sessionid";
                    trans.get_sessiondata_by_sessionid_hash(&hash)}?
                {
                    if let Some(user_id) = sessiondata.user_id {
                        if trans.user_in_group(user_id, group_id)? {
                            // Update timestamp; OK to only update it here?
                            sessiondata.last_request_time = now_unixtime();
                            trans.update_sessiondata(&sessiondata)?;
                            Ok(LoginState::Allowed)
                        } else {
                            Ok(LoginState::NotAllowed)
                        }
                    } else {
                        Ok(LoginState::NeedLogin)
                    }
                } else {
                    Ok(LoginState::NeedLogin)
                }
            })?;
            match state {
                LoginState::NeedLogin => {
                    let target = AUriLocal::from_str(
                        "/login",
                        Some(QueryString::new(
                            [("return_path", context.path_str())])));
                    Ok(Some(Response::redirect_302(String::from(target)).into()))
                }
                LoginState::NotAllowed => {
                    show_popup_box_page(
                        context, html, &style,
                        PopupBoxKind::Error(HttpResponseStatusCode::Forbidden403),
                        html.str("Permission denied")?,
                        html.str("You are not allowed to access this resource.")?,
                    ).map(|o| o.map(AResponse::from))
                }
                LoginState::Allowed => self.call(context, method, path, html)
            }
        }))
    }
}


/// To be instantiated for `/` (or similar?), will redirect to
/// e.g. `/en.html` using the lang from the current `AContext`.
pub fn language_handler<L: Language + 'static>(
) -> Arc<dyn Handler<L>> {
    Arc::new(ExactFnHandler::new(    
        move |
        context: &AContext<L>,
        _method: HttpRequestMethodSimple,
        _html: &HtmlAllocator
            | -> Result<AResponse>
        {
            let lang = context.lang();
            let target = PPath::from_str(&format!("/{}.html", lang.as_str()));
            Ok(context.redirect_302_with_query(&target).into())
        }))
}
