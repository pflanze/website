use std::{sync::Arc,
          path::{Path, PathBuf},
          time::{Duration, SystemTime},
          fs::read_dir,
          thread,
          panic::catch_unwind};
use anyhow::{Result, anyhow, bail, Context};
use chrono::NaiveDate;
use kstring::KString;

use crate::{ahtml::{SerHtmlFrag, HtmlAllocator, AllocatorPool, AVec, Node, att},
            router::UniqueRouter,
            util::first_and_rest,
            markdown::{MarkdownFile, StylingInterface},
            conslist::{List, cons},
            path::{extension_eq, base, IntoBoxPath},
            miniarcswap::MiniArcSwap,
            cmpfilemeta::{CmpFileMeta, GetCmpFileMeta},
            easyfiletype::EasyFileType,
            loop_try,
            trie::Trie,
            try_option,
            try_result,
            myfrom::MyFrom, option_util::TryMap};
use crate::{nodt as dt, time, notime};
use crate::ahtml::{H2_META, P_META};

#[derive(Debug, Clone)]
pub struct Breadcrumb {
    // Evil, they are, URLs. Because this is preserialized, can't mod
    // the contained links later (and don't want to even think about
    // preserialized-with-holes), so we pregenerate both variants.
    without_slash: Arc<SerHtmlFrag>,  // for "2023/10/23" urls
    with_slash: Arc<SerHtmlFrag>      // for "2023/10/23/" urls
}
impl Breadcrumb {
    pub fn with_slash(&self, with_slash: bool) -> Arc<SerHtmlFrag> {
        if with_slash {
            &self.with_slash
        } else {
            &self.without_slash
        }.clone()
    }
}

#[derive(Debug, Clone)]
pub struct BlogPost {
    pub cmpfilemeta: CmpFileMeta,
    pub publish_date: NaiveDate, // parsed from file path
    pub title_plain: KString,
    pub title_html: Arc<SerHtmlFrag>,
    /// The table of contents
    pub toc: Arc<SerHtmlFrag>,
    /// The part before the first header, or the first paragraph (teaser)
    pub lead: Option<Arc<SerHtmlFrag>>,
    /// The part after the lead
    pub main: Arc<SerHtmlFrag>,
    pub num_footnotes: usize,
    pub footnotes: Arc<SerHtmlFrag>,
    pub breadcrumb: Breadcrumb,
}
impl BlogPost {
    // XX todo: use time from Git, not mtime!
    pub fn modified(&self) -> SystemTime {
        self.cmpfilemeta.modified_time
    }
}

#[derive(Debug, Clone)]
pub struct BlogPostIndex {
    // None for top level index
    pub breadcrumb: Option<Breadcrumb>,
}

#[derive(Debug)]
pub enum BlogNode {
    BlogPost(BlogPost),
    BlogPostIndex(BlogPostIndex)
}

impl BlogNode {
    fn blogpost(&self) -> Option<&BlogPost> {
        match self {
            BlogNode::BlogPost(p) => Some(p),
            BlogNode::BlogPostIndex(_) => None
        }
    }
}


#[derive(Debug)]
pub struct BlogCache {
    pub router: UniqueRouter<BlogNode>,
}

pub enum ParsedDatePart {
    Integer(u16),
    NaiveDate(NaiveDate),
}

pub struct ParsedContextFrame<'f> {
    filename: &'f str,
    parseddatepart: ParsedDatePart,
}

type ContextFrameParser = for<'f> fn(&str, &List<ParsedContextFrame<'f>>)
                                     -> Option<ParsedDatePart>;

fn parse_year<'f>(
    s: &str, _prev: &List<ParsedContextFrame<'f>>
) -> Option<ParsedDatePart> {
    if s.len() != 4 { return None }
    let n: u16 = s.parse().ok()?;
    if n < 1980 { return None }
    if n > 2150 { return None }
    Some(ParsedDatePart::Integer(n))
}
fn parse_month<'f>(
    s: &str, _prev: &List<ParsedContextFrame<'f>>
) -> Option<ParsedDatePart> {
    if s.len() != 2 { return None }
    let n: u16 = s.parse().ok()?;
    if n < 1 { return None }
    if n > 12 { return None }
    Some(ParsedDatePart::Integer(n))
}
fn parse_day<'f>(
    s: &str, prev: &List<ParsedContextFrame<'f>>
) -> Option<ParsedDatePart> {
    if s.len() != 2 { return None }
    let n: u16 = s.parse().ok()?;
    if n < 1 { return None }
    if n > 31 { return None }

    let month = prev.first().expect("CONTEXT ensures it");
    let year = prev.rest().and_then(|l| l.first()).expect("CONTEXT ensures it");
    match (&year.parseddatepart, &month.parseddatepart) {
        (ParsedDatePart::Integer(year), ParsedDatePart::Integer(month)) => {
            let d = NaiveDate::from_ymd_opt(
                *year as i32,
                *month as u32,
                n as u32)?;
            Some(ParsedDatePart::NaiveDate(d))
        }
        _ => None
    }
}


struct ContextFrame(&'static str, ContextFrameParser);

static CONTEXT: &'static [ContextFrame] = &[
    ContextFrame("year", parse_year),
    ContextFrame("month", parse_month),
    ContextFrame("valid day value", parse_day),
];


fn breadcrumbhtml<'f>(
    html: &HtmlAllocator,
    parsed_context: &'f List<ParsedContextFrame<'f>>,
    top_relpath: &str, // "." or ".."
) -> Result<Arc<SerHtmlFrag>> {
    let mut v: AVec<Node> = html.new_vec();
    let mut l = parsed_context;
    let mut uplink = String::from(top_relpath);
    loop {
        match l {
            List::Pair(a, r) => {
                v.push(
                    html.li(
                        [att("class", "breadcrumb_item")],
                        [
                            html.a(
                                [att("href", &uplink)],
                                [html.str(a.filename)?])?
                        ])?)?;
                l = r;
                uplink.push_str("/..");
            }
            List::Null => break
        }
    }
    v.reverse();
    Ok(
        Arc::new(
            html.preserialize(
                html.div(
                    [att("class", "breadcrumb")],
                    [
                        html.ul(
                            [],
                            v.as_slice())?
                    ])?)?))
}

fn breadcrumb<'f>(
    html: &HtmlAllocator,
    parsed_context: &'f List<ParsedContextFrame<'f>>,
) -> Result<Breadcrumb> {
    Ok(Breadcrumb {
        without_slash: breadcrumbhtml(html, parsed_context, ".")?,
        with_slash: breadcrumbhtml(html, parsed_context, "..")?,
    })
}

// Walk the file system, copying over entries from oldleaf if
// available and matching (unchanged `CmpFilemeta`)
fn populate<'f, 'c>(
    leaf: &mut Trie<BlogNode>,
    oldleaf: Option<&Trie<BlogNode>>,
    coming_context: &'c [ContextFrame], // part of CONTEXT
    parsed_context: &'f List<ParsedContextFrame<'f>>,
    fsdirpath: &Path,
    fsbasepath: &Path,
    html: &HtmlAllocator,
    style: &dyn StylingInterface,
) -> Result<()> {
    dt!("populate", fsdirpath);

    // Add index as the endpoint for this node
    {
        let breadcrumb =
            if let Some(parent_context) = parsed_context.rest() {
                Some(breadcrumb(html, parent_context)?)
            } else {
                None
            };
        let endp = leaf.endpoint_mut().expect("always allowed due to `true`");
        *endp = Some(BlogNode::BlogPostIndex(BlogPostIndex {
            breadcrumb
        }));
    }

    let items =
        read_dir(fsdirpath).with_context(
            || anyhow!("read_dir on {:?}", fsdirpath))?
        .into_iter().map(
            |direntry| -> Result<_> {
                let direntry = direntry?;
                // Make sure the file name is UTF-8 to prevent
                // problems with trying to send URLs containing other
                // byte sequences to the browser.
                let filename = direntry.file_name().into_string().ok().ok_or_else(
                    || anyhow!("Blog under {fsdirpath:?}: item can't be \
                                converted to string: {:?}",
                               direntry.file_name().to_string_lossy()))?;
                let mut fspath: PathBuf = fsdirpath.into();
                fspath.push(&filename);
                let x = fspath.symlink_metadata()?.cmpfilemeta()?;
                Ok((filename,
                    fspath,
                    x))
            });

    for item in items {
        let (filename, fspath, cmpfilemeta) = item?;
        let fspath_lossy = KString::myfrom(fspath.to_string_lossy());
        match try_result!{
            macro_rules! leafs_for_recursion {
                ($origfilename:expr) => {{
                    // Generate KString early since we need it anyway
                    // and this might avoid constructing it twice
                    // (forgot how trait works and too lazy to check now):
                    let filename = KString::from_ref(&$origfilename);
                    let oldleaf2 = oldleaf.and_then(
                        |l| l.get_leaf(&[&filename]));
                    let leaf2 = leaf.get_leaf_mut(&[filename]).expect(
                        "allow_both are set so it never fails");
                    (oldleaf2, leaf2)
                }}
            }
            Ok(match cmpfilemeta.easyfiletype {
                EasyFileType::Dir => {
                    let (ContextFrame(desc, parse), rest_context)
                        = first_and_rest(coming_context).ok_or_else(
                            || anyhow!("invalid blog subdirectory at {fspath:?} is too deep"))?;
                    if let Some(pdp) = parse(&filename, parsed_context) {
                        let (oldleaf2, leaf2) = leafs_for_recursion!(filename);
                        let ctx = cons(
                            ParsedContextFrame {
                                filename: &filename,
                                parseddatepart: pdp,
                            },
                            parsed_context);
                        populate(
                            leaf2,
                            oldleaf2,
                            rest_context,
                            &ctx,
                            &fspath,
                            fsbasepath,
                            html,
                            style)?;
                    } else {
                        bail!("invalid blog subdirectory at {fspath:?}: \
                               expected {desc} as the filename part");
                    }
                },
                EasyFileType::File => {
                    // ^ skips symlinks due to using symlink_metadata()

                    if extension_eq(&fspath, "md") {
                        dt!("mdfile", fspath);

                        let filename_html =
                            format!("{}.html",
                                    base(&filename).expect(
                                        "shown above to have suffix"));

                        let (oldleaf2, leaf2) = leafs_for_recursion!(filename_html);

                        // Re-use cached BlogPost?
                        let reuse_blogpost = try_option! {
                            let oldblogpost = oldleaf2?.endpoint()?.blogpost()?;
                            if oldblogpost.cmpfilemeta == cmpfilemeta {
                                Some(oldblogpost)
                            } else {
                                None
                            }
                        };

                        let blogpost =
                            if let Some(blogpost) = reuse_blogpost {
                                (*blogpost).clone()
                                // ^ ~cheap since it contains just Arc's
                                // and some small fields (CmpFileMeta is
                                // about 5 words).
                            } else {
                                time!{
                                    fspath.to_string_lossy();

                                    let publish_date =
                                        match parsed_context {
                                            List::Pair(a, _) =>
                                                match a.parseddatepart {
                                                    ParsedDatePart::Integer(_) =>
                                                        bail!(
                                                            "missing parsed publish date, \
                                                             blog post must be in a dir with \
                                                             path yyyy/mm/dd"),
                                                    ParsedDatePart::NaiveDate(d) => d,
                                                },
                                            List::Null => bail!(
                                                "missing parsed_context, \
                                                 blog post must be in a dir with \
                                                 path yyyy/mm/dd"),
                                        };

                                    let mf = MarkdownFile::new(fspath);
                                    let pmd = mf.process_to_html(html)?;
                                    let fixed_body = pmd.fixed_html(html)?;
                                    let (lead, main) = {
                                        let bodynode = html.get_node(fixed_body).expect(
                                            "guaranteed");
                                        let elt = bodynode.as_element().ok_or_else(
                                            || anyhow!("guaranteed to be an element, no?"))?;
                                        if elt.attr().len() != 0 {
                                            bail!("guaranteed to not have atts, no?")
                                        }
                                        let bodyslice = elt.body().clone();
                                        drop(bodynode);
                                        let div = |slice| html.div([], slice);
                                        let no_lead = || -> Result<_> {
                                            Ok((None, div(bodyslice)?))
                                        };
                                        if let Some((lead, main)) = bodyslice.split_when(
                                            |id| {
                                                if let Some(e) = html.get_node(id)
                                                    .expect("guaranteed").as_element()
                                                {
                                                    e.meta == *H2_META
                                                } else {
                                                    false
                                                }
                                            },
                                            html) {
                                            (Some(div(lead)?), div(main)?)
                                        } else if let Some((first, rest)) =
                                            bodyslice.first_and_rest(html)
                                        {
                                            let firstnode = html.get_node(first).expect(
                                                "guaranteed");
                                            if let Some(e) = firstnode.as_element() {
                                                if e.meta == *P_META {
                                                    drop(firstnode);
                                                    (Some(first), div(rest)?)
                                                } else {
                                                    no_lead()?
                                                }
                                            } else {
                                                no_lead()?
                                            }
                                        } else {
                                            no_lead()?
                                        }
                                    };
                                    let title =
                                        if let Some(slice) = pmd.meta().title() {
                                            html.span([], slice)?
                                        } else {
                                            eprintln!(
                                                "markdown document is missing a \
                                                 title: {:?}", mf.path());
                                            html.span(
                                                [],
                                                [html.str("(missing title)")?])?
                                        };
                                    let toc = pmd.meta().toc_html_fragment(html)?;
                                    let (num_footnotes, footnotes) =
                                        pmd.meta().footnotes_html_fragment(html, style)?;

                                    BlogPost {
                                        cmpfilemeta,
                                        publish_date,
                                        title_plain:
                                        html.to_plain_string(title)?,
                                        title_html:
                                        Arc::new(html.preserialize(title)?),
                                        toc:
                                        Arc::new(html.preserialize(toc)?),
                                        lead:
                                        lead.try_map(|id| -> Result<_> {
                                            Ok(Arc::new(html.preserialize(id)?))
                                        })?,
                                        main:
                                        Arc::new(html.preserialize(main)?),
                                        num_footnotes,
                                        footnotes:
                                        Arc::new(html.preserialize(footnotes)?),
                                        breadcrumb:
                                        breadcrumb(html, parsed_context)?,
                                    }
                                }
                            };

                        let opt_entry = leaf2.endpoint_mut()?;
                        if opt_entry.is_none() {
                            *opt_entry = Some(BlogNode::BlogPost(blogpost));
                        } else {
                            panic!("can't have the same path in the file system \
                                    multiple times")
                        }
                    }
                },
                EasyFileType::Symlink => (),
                EasyFileType::Other => (),
            })
        } {
            Ok(()) => (),
            Err(e) => {
                bail!("blog::populate: Error: {}, processing path {:?}",
                          e, &*fspath_lossy)
            }
        }
    }

    Ok(())
}

impl BlogCache {
    fn new() -> BlogCache {
        BlogCache {
            router: UniqueRouter::new(true),
        }
    }
    
    /// Needs an Allocator but only temporarily, BlogCache does not contain
    /// AId:s but only preserialized HTML.
    fn from_dir(
        basepath: &Path,
        oldtrie: Option<&Trie<BlogNode>>, // for the same basepath, please
        html: &HtmlAllocator,
        style: &dyn StylingInterface
    ) -> Result<BlogCache> {
        notime!{
            "BlogCache::from_dir";
            let mut blogcache = BlogCache::new();
            populate(blogcache.router.trie_mut(),
                     oldtrie,
                     CONTEXT,
                     &List::Null,
                     basepath,
                     basepath,
                     html,
                     style)?;
            Ok(blogcache)
        }
    }
}

pub struct Blog {
    basepath: Box<Path>,
    blogcache: MiniArcSwap<BlogCache>,
    style: Arc<dyn StylingInterface>,
    allocpool: &'static AllocatorPool,
    // ^ go Arc instead of 'static? -- XX not even needed, just have
    // updater_thread have it, handlers will get it anyway
    // updater_thread: JoinHandle<()>,
}

impl Blog {
    pub fn open<P: IntoBoxPath>(
        basepath: P,
        allocpool: &'static AllocatorPool,
        style: Arc<dyn StylingInterface>
    ) -> Result<Arc<Blog>>
    {
        let basepath = basepath.into_box_path();
        let blogcache = {
            let mut allocguard = allocpool.get();
            Arc::new(BlogCache::from_dir(&basepath,
                                         None,
                                         allocguard.allocator(),
                                         &*style)?)
        };
        let blog = Arc::new(Blog {
            basepath: basepath.into_box_path(),
            blogcache: MiniArcSwap::new(blogcache),
            allocpool,
            style,
        });
        let _updater_thread =
            thread::Builder::new().name("blog_updater".into()).spawn({
                let blog = Arc::clone(&blog);
                move || -> ! {
                    loop_try! {
                        thread::sleep(Duration::from_millis(400));
                        match catch_unwind(|| -> Result<()> {
                            let oldblogcache = blog.blogcache.get();
                            let mut allocguard = blog.allocpool.get();
                            let newblogcache = BlogCache::from_dir(
                                &blog.basepath,
                                Some(oldblogcache.router.trie()),
                                allocguard.allocator(),
                                &*blog.style)?;
                            // ah, and need a way to know if new? actually
                            // doesn't matter, just publish it:
                            blog.blogcache.set(Arc::new(newblogcache));
                            Ok(())
                        }) {
                            Ok(Ok(())) => Ok(()),
                            Ok(Err(e)) => Err(e),
                            Err(e) => {
                                Err(anyhow!("updater thread: caught panic: {e:?}"))
                            }
                        }
                    }
                }
            })?;
        Ok(blog)
    }

    pub fn blogcache(&self) -> Arc<BlogCache> {
        self.blogcache.get()
    }
}

