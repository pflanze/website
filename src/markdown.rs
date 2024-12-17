//! Convert markdown to HTML.

use std::{path::PathBuf, fmt::{Display, Debug}, collections::HashMap, panic::RefUnwindSafe};
use anyhow::{Result, anyhow, bail};
use backtrace::Backtrace;
use html5gum::{Token, HtmlString};
use kstring::KString;
use pulldown_cmark::{Parser, Options, Event, Tag, HeadingLevel, LinkType};

use ahtml::{AId, HtmlAllocator, Node, AVec, P_META,
                     H1_META, H2_META, H3_META, H4_META, H5_META, H6_META,
                     DIV_META, OL_META, UL_META, LI_META, PRE_META,
                     BLOCKQUOTE_META, TABLE_META, TH_META, TR_META,
                     TD_META, EM_META, STRONG_META, S_META, METADB, ASlice, Print,
                     TITLE_META, Element, Flat,
            att};
use ahtml_html::meta::ElementMeta;

use chj_util::{nowarn_todo as warn_todo,
               nowarn as warn,
               nodt as dt};

use crate::{webutils::email_url,
            util::{infinite_sequence, autovivify_last, enum_name},
            try_option,
            io_util::my_read_to_string,
            myfrom::kstring_myfrom2};

fn error_not_an_html5_tag_name(name: &str) -> anyhow::Error {
    anyhow!("not an HTML5 tag name: {name:?}\n{:?}",
            Backtrace::new())
}

/// This can't be replaced with `att` or the MyFrom trait, because it
/// can fail.
fn kstring(s: HtmlString) -> Result<KString> {
    Ok(KString::from_string(String::from_utf8(s.0)?))
}


// ------------------------------------------------------------------
// Formatting parametrization

pub trait StylingInterface: Send + Sync + RefUnwindSafe {
    fn new_context<'c>(
        &'c self,
        html: &HtmlAllocator,
    ) -> Result<Box<dyn StylingContextInterface<'c> + 'c>>;
}

pub trait StylingContextInterface<'c> {
    fn format_footnote_definition(
        &self,
        html: &HtmlAllocator,
        reference: &Footnoteref,
        backreferences: &[Backref],
        clean_slice: &ASlice<Node>,
    ) -> Result<Flat<Node>>;

    fn format_footnotes(
        &self,
        body: ASlice<Node>,
        html: &HtmlAllocator,
    ) -> Result<AId<Node>>;
}

// ------------------------------------------------------------------


fn elementmeta_from_headinglevel(level: HeadingLevel) -> &'static ElementMeta {
    match level {
        HeadingLevel::H1 => *H1_META,
        HeadingLevel::H2 => *H2_META,
        HeadingLevel::H3 => *H3_META,
        HeadingLevel::H4 => *H4_META,
        HeadingLevel::H5 => *H5_META,
        HeadingLevel::H6 => *H6_META,
    }
}

fn elementmeta_from_num(level: i32) -> Option<&'static ElementMeta> {
    match level {
        1 => Some(*H1_META),
        2 => Some(*H2_META),
        3 => Some(*H3_META),
        4 => Some(*H4_META),
        5 => Some(*H5_META),
        6 => Some(*H6_META),
        _ => None
    }
}

// Returning a signed integer so that calculating with differences is
// easy.
fn headinglevel_num(level: HeadingLevel) -> i32 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn level_from_elementmeta(meta: &'static ElementMeta) -> Option<i32> {
    if meta == *H1_META { Some(1) }
    else if meta == *H2_META { Some(2) }
    else if meta == *H3_META { Some(3) }
    else if meta == *H4_META { Some(4) }
    else if meta == *H5_META { Some(5) }
    else if meta == *H6_META { Some(6) }
    else { None }
}

fn text_to_anchor(s: &str, res: &mut String) {
    let mut last_was_space = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            res.push(c.to_ascii_lowercase());
            last_was_space = false;
        } else if c.is_whitespace() {
            if !last_was_space {
                res.push('-');
            }
        } else {
            res.push('_');
            last_was_space = false;
        }
    }
}

pub struct MarkdownFile {
    path: PathBuf
}

pub struct MarkdownHeader {
    /// The body of the <hX> element
    html: ASlice<Node>,
    anchor_name: KString,
}

pub struct MarkdownHeading {
    /// Original level as per .md document, used for building up (won't
    /// correspond to the HTML any more after fixing that up).
    level: HeadingLevel,
    header: Option<MarkdownHeader>,
    subheadings: Vec<MarkdownHeading>,
}

impl MarkdownHeading {
    fn append_heading(&mut self, our_level: u32, h: MarkdownHeading) {
        if h.level as u32 == our_level {
            self.subheadings.push(h)
        } else {
            autovivify_last(
                &mut self.subheadings,
                || MarkdownHeading {
                    level: HeadingLevel::try_from(our_level as usize).expect(
                        "must exist because h.level is yet larger"),
                    header: None,
                    subheadings: Vec::new()
                }).append_heading(our_level + 1, h)
        }
    }

    fn to_toc_html_fragment(
        &self, html: &HtmlAllocator
    ) -> Result<AId<Node>> {
        let mut body = html.new_vec();
        for subheading in &self.subheadings {
            body.push(subheading.to_toc_html_fragment(html)?)?;
        }
        html.dl(
            [],
            [
                if let Some(header) = &self.header {
                    let mut anchor = String::new(); // cache?
                    anchor.push_str("#");
                    anchor.push_str(&header.anchor_name);
                    html.dt(
                        [],
                        [
                            html.a(
                                [att("href", anchor)],
                                // Should we actually strip HTML markup?
                                &header.html
                                )?
                        ])?
                } else {
                    html.dt(
                        [], 
                        [])?
                },
                html.dd(
                    [],
                    body)?
            ])
    }

    // Again duplication with method in MarkdownMeta. Stupid. todo clean up?
    fn top_heading_level(&self) -> Option<HeadingLevel> {
        if self.header.is_some() {
            Some(self.level)
        } else {
            self.subheadings.iter().filter_map(
                |heading| heading.top_heading_level()).max()
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Backref(pub u32);

impl Backref {
    pub fn to_kstring(&self, with_hash: bool) -> KString {
        KString::from_string(format!("{}footnoteref-{}",
                                     if with_hash { "#" } else {""},
                                     self.0))
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Footnoteref(pub u32);

impl Footnoteref {
    pub fn to_kstring(&self, with_hash: bool) -> KString {
        KString::from_string(format!("{}footnote-{}",
                                     if with_hash { "#" } else {""},
                                     self.0))
    }
}

struct FootnoteDefinition {
    reference: Option<Footnoteref>,
    text: Option<ASlice<Node>>,
    /// places with references to this definition, in order of
    /// appearance in document
    backreferences: Vec<Backref>,
}

pub struct MarkdownMeta {
    /// contents of <title> tag only (deriving from headers happens
    /// outside)
    title: Option<ASlice<Node>>,
    headings: Vec<MarkdownHeading>,
    /// footnote label to definition
    footnotes: HashMap<KString, FootnoteDefinition>,
}
impl MarkdownMeta {
    fn new() -> MarkdownMeta {
        MarkdownMeta {
            title: None,
            headings: Vec::new(),
            footnotes: HashMap::new(),
        }
    }

    fn push_heading(&mut self, h: MarkdownHeading) {
        match h.level {
            HeadingLevel::H1 => self.headings.push(h),
            _ => autovivify_last(
                &mut self.headings,
                || MarkdownHeading {
                    level: HeadingLevel::H1,
                    header: None,
                    subheadings: Vec::new()
                }).append_heading(2, h)
        }
    }

    // Stupid modified copy-paste[aha, from to_toc_html_fragment],
    // "todo really unify the two DS;
    // actually easy just None header ?"-- but now ~happy
    // with it, OK? &Vec<MarkdownHeading> is now the thing to be generic on?
    // Alright, should then do function on *that* ^, todo?
    pub fn toc_html_fragment(
        &self, html: &HtmlAllocator
    ) -> Result<AId<Node>> {
        let headings = self.title_and_remaining_headings().1;
        let mut body = html.new_vec();
        for subheading in headings {
            body.push(subheading.to_toc_html_fragment(html)?)?;
        }
        // Using `div` here instead of `dl` is wrong in that multiple
        // toplevel entries will be separate now. But what would the
        // `dt`? Empty? It would indent the `dd` holding `body`. Do it
        // iff there are >1 body nodes? Perennial question about what
        // '#' header should mean in Markdown.
        if true {
            html.div([att("class", "toc_wrapper")], body)
        } else {
            html.dl(
                [],
                [
                    html.dt([], [])?,
                    html.dd([], body)?
                ])
        }
    }

    // XX why not just preserialize the individual footnote
    // definitions, and leave formatting of the rest to blog.rs?
    // Checking for missing definitions should perhaps still be done
    // in markdown.rs, though.
    pub fn footnotes_html_fragment(
        &self,
        html: &HtmlAllocator,
        style: &dyn StylingInterface,
    ) -> Result<(usize, AId<Node>)> {
        let mut footnotes: Vec<_> = self.footnotes.iter().collect();
        footnotes.sort_by_key(|f| f.1.reference);
        // dbg!(&footnotes);

        let context = style.new_context(html)?;
        let mut body = html.new_vec();
        for (label, fnd) in &footnotes {
            let reference = fnd.reference.ok_or_else(
                || anyhow!("unused footnote {:?}", label.as_str()))?;
            let slice = fnd.text.ok_or_else(
                || anyhow!("missing definition for footnote {:?}", label.as_str()))?;
            let clean_slice = slice.unwrap_element(*P_META, true, html);
            body.push_flat(
                context.format_footnote_definition(
                    html,
                    &reference,
                    &fnd.backreferences,
                    &clean_slice,
                )?,
                html)?;
        }
        Ok((footnotes.len(),
            context.format_footnotes(body.as_slice(), html)?))
    }

    /// Split title/header hierarchy into title and rest; takes
    /// `<title>` if available by preference, otherwise the first
    /// heading if it's a '#' and there are no other '#' ones. The
    /// last returned value is true if a heading from the markdown
    /// file was skipped (i.e. it needs to be dropped from the
    /// generated HTML to avoid header duplication).
    pub fn title_and_remaining_headings(&self)
       -> (Option<&ASlice<Node>>, &Vec<MarkdownHeading>, bool)
    {
        if let Some(title) = &self.title {
            (Some(title), &self.headings, false)
        } else {
            if let Some(header) = try_option!{
                if self.headings.len() != 1 { return None; }
                self.headings[0].header.as_ref()
            } {
                (Some(&header.html), &self.headings[0].subheadings, true)
            } else {
                (None, &self.headings, false)
            }
        }
    }

    /// The contents of an optional single `<title>` element,
    /// or if missing, the first heading if it's a
    /// '#' and there are no other '#' ones.
    pub fn title(&self) -> Option<&ASlice<Node>> {
        self.title_and_remaining_headings().0
    }

    /// Like `title` but as a string with markup stripped, and falling
    /// back to `alternative` if not present.
    pub fn title_string(&self, html: &HtmlAllocator, alternative: &str)
                        -> Result<KString>
    {
        if let Some(sl) = self.title() {
            let mut v = String::new();
            sl.print_plain(&mut v, html)?;
            Ok(KString::from_string(v))
        } else {
            Ok(KString::from_ref(alternative))
        }
    }

    fn top_heading_level(&self) -> Option<HeadingLevel> {
        self.headings.iter().filter_map(
            |heading| heading.top_heading_level()).max()
    }
}

/// The result of processing a markdown file.
pub struct ProcessedMarkdown {
    /// Conversion to html of the text, with the original heading
    /// levels translated to identical HTML levels (may need fixing up
    /// before serving).
    html: AId<Node>,
    /// Metadata extracted also during the conversion.
    meta: MarkdownMeta,
}

impl ProcessedMarkdown {
    pub fn html(&self) -> AId<Node> { self.html }
    pub fn meta(&self) -> &MarkdownMeta { &self.meta }

    pub fn fixed_html(&self, html: &HtmlAllocator) -> Result<AId<Node>> {
        // Which is the top level we *want*?
        let (opt_title, _heading, do_drop_h1) =
            self.meta.title_and_remaining_headings();
        dt!(&format!("fixed_html {:?}",
                     opt_title.map_or_else(
                         || Ok(String::from("(no title)")),
                         |t| t.to_string(html))));
        // We want to either drop H1 in the document and not shift
        // anything (because H1 existed and was the only H1 header,
        // after dropping it the next level can only be H2 or less and
        // we leave it at what remains), or, shift them if necessary
        // so that the top level becomes H2. Unless it couldn't
        // extract a title, in which case we leave the document
        // untouched.
        if opt_title.is_none() {
            warn!("no title could be derived");
            return Ok(self.html)
        }
        let fixup: Box<dyn Fn(_) -> _> =
            if do_drop_h1 {
                warn!("do_drop_h1");
                Box::new(|id: AId<Node>| -> Result<Option<AId<Node>>> {
                    let node = html.get_node(id).expect("correct Allocator");
                    if let Some(elt) = node.as_element() {
                        if elt.meta() == *H1_META {
                            Ok(None)
                        } else {
                            Ok(Some(id))
                        }
                    } else {
                        Ok(Some(id))
                    }
                })
            } else {
                if let Some(top_level_have) = self.meta.top_heading_level() {
                    let top_level_want = 2; // HeadingLevel::H2;
                    let diff = top_level_want - headinglevel_num(top_level_have);
                    warn!("diff = {diff}");
                    if diff == 0 {
                        return Ok(self.html)
                    }
                    Box::new(move |id: AId<Node>| -> Result<Option<AId<Node>>> {
                        let node = html.get_node(id).expect("correct Allocator");
                        if let Some(elt) = node.as_element() {
                            if let Some(lvl) = level_from_elementmeta(elt.meta()) {
                                let lvl2 = lvl + diff;
                                let meta2 = elementmeta_from_num(lvl2).ok_or_else(
                                    || anyhow!("can't shift header levels by {diff} \
                                                because getting out of range"))?;
                                let elt2 = Element {
                                    meta: meta2,
                                    attr: elt.attr().clone(),
                                    body: elt.body().clone()
                                };
                                drop(node);
                                Ok(Some(html.allocate_element(elt2)?))
                            } else {
                                Ok(Some(id))
                            }
                        } else {
                            Ok(Some(id))
                        }
                    })
                } else {
                    warn!("no headings, thus noop");
                    return Ok(self.html)
                }
            };

        let node2 = {
            let elt = {
                let node = html.get_node(self.html).expect(
                    "ProcessedMarkdown to be used with the same Allocator it was created with");
                // Bummer, Element is quite large (5 words?), but we have
                // to free up the borrow from get_node because
                // try_filter_map_body needs a writable one.
                (*node.try_element()?).clone()
            };
            elt.try_filter_map_body::<Node>(fixup, html)?
        };
        Ok(html.allocate_element(node2)?)
    }
}

// Internals for impl MarkdownFile:

#[derive(Debug)]
enum ContextTag<'t> {
    Markdown(Tag<'t>),
    Html(&'static ElementMeta),
}

impl<'t> Display for ContextTag<'t> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextTag::Markdown(tag) =>
                f.write_fmt(format_args!(
                    "Markdown {:?} scope", enum_name(tag))),
            ContextTag::Html(meta) =>
                f.write_fmt(format_args!(
                    "HTML {:?} element", meta.tag_name.as_str())),
        }
    }
}

impl<'t> PartialEq for ContextTag<'t> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ContextTag::Markdown(a), ContextTag::Markdown(b)) =>
                a == b,
            (ContextTag::Html(a), ContextTag::Html(b)) =>
                std::ptr::eq(*a, *b),
            _ => false
        }
    }
}

impl<'t> ContextTag<'t> {
    fn assert_eq(&self, other: &ContextTag) -> Result<()> {
        if *self == *other {
            Ok(())
        } else {
            Err(anyhow!("non-balanced tags/markup: {} ending as {}",
                        self, other))
        }
    }
}

struct ContextFrame<'a, 't> {
    tag: ContextTag<'t>,
    // meta: &'static ElementMeta, -- no, given ad-hoc on closing
    // event.
    atts: AVec<'a, (KString, KString)>,
    body: AVec<'a, Node>,
    last_footnote_reference: Option<u32>, // last index into body holding one
}


impl MarkdownFile {
    pub fn new(path: PathBuf) -> MarkdownFile {
        MarkdownFile { path } 
    }
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
    
    /// Convert to HTML, and capture metainformation to allow for
    /// creation of TOC and footnotes section.
    pub fn process_to_html(
        &self, html: &HtmlAllocator
    ) -> Result<ProcessedMarkdown>
    {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_SMART_PUNCTUATION);// XX config
        options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

        // `Parser` is NOT supporting streaming. For reasons of
        // shining in (superficial) performance bencharks?
        // XX impose a size limit on the markdown file here?
        let s = my_read_to_string(&self.path)?;
        let mut parser = Parser::new_ext(&s, options);

        // Context
        let mut _context: Vec<ContextFrame> = Vec::new();
        let mut context = &mut _context;
        // Push a base frame (wrapper around everything):
        context.push(ContextFrame {
            tag: ContextTag::Markdown(Tag::Paragraph), // fake
            atts: AVec::new(html),
            body: AVec::new(html),
            last_footnote_reference: None,
        });
        macro_rules! new_contextframe {
            ($tag:expr) => {
                ContextFrame {
                    tag: $tag,
                    atts: AVec::new(html),
                    body: AVec::new(html),
                    last_footnote_reference: None,
                }
            }
        }

        // Opening a context
        macro_rules! mdopen {
            ($tag:expr) => {
                context.push(new_contextframe!(ContextTag::Markdown($tag)))
            }
        }

        // Closing a context
        let frame_to_element =
            |frame: ContextFrame, meta: &'static ElementMeta| -> Result<AId<Node>> {
                html.new_element(
                    meta,
                    frame.atts.as_slice(),
                    frame.body.as_slice())
            };
        let close =
            |
        context: &mut Vec<ContextFrame>,
        tag: ContextTag,
        meta: &'static ElementMeta
            | -> Result<()>
        {
            let frame = context.pop().expect("start before end");
            frame.tag.assert_eq(&tag)?;
            let outerframe = context.last_mut()
                .expect("at least base frame");
            outerframe.body.push(frame_to_element(frame, meta)?)?;
            Ok(())
        };
        macro_rules! mdclose {
            ($tag:expr, $meta:expr) => {
                close(&mut context, ContextTag::Markdown($tag), $meta)
            }
        }
        // Alternative approach:
        macro_rules! pop {
            ($tag:expr) => {{
                // XX minimize code via local function
                let frame = context.pop().expect("start before end");
                frame.tag.assert_eq(&$tag)?;
                let outerframe = context.last_mut()
                    .expect("at least base frame");
                (frame.atts, frame.body, outerframe)
            }}
        }
        macro_rules! mdpop {
            ($tag:expr) => {
                pop!(ContextTag::Markdown($tag))
            }
        }

        macro_rules! current_frame {
            () => {
                context.last_mut().expect(
                    "At least base frame; at least bug in markdown lib?")
            }
        }

        let mut markdownmeta =  MarkdownMeta::new();
        // let mut current_heading = None;
        let mut anchor_name = String::new();
        let mut tmp = String::new();
        // Anchor names to number of uses, acting as id
        let mut anchor_names: HashMap<KString, u32> = HashMap::new();
        
        let mut next_footnote_number = infinite_sequence(1, 1);
        let mut next_footnote_backreference = infinite_sequence(1, 1);

        while let Some(item) = parser.next() {
            match item {
                Event::Start(x) =>
                    match x {
                        Tag::Paragraph =>
                            mdopen!(Tag::Paragraph),
                        Tag::Heading(level, fragmentid, classes) =>
                            mdopen!(Tag::Heading(level, fragmentid, classes)),
                        Tag::BlockQuote =>
                            mdopen!(Tag::BlockQuote),
                        Tag::CodeBlock(kind) =>
                            mdopen!(Tag::CodeBlock(kind)),
                        Tag::List(firstitemnum) =>
                            mdopen!(Tag::List(firstitemnum)),
                        Tag::Item =>
                            mdopen!(Tag::Item),
                        Tag::FootnoteDefinition(label) =>
                            mdopen!(Tag::FootnoteDefinition(label)),
                        Tag::Table(alignments) =>
                            mdopen!(Tag::Table(alignments)),
                        Tag::TableHead =>
                            mdopen!(Tag::TableHead),
                        Tag::TableRow => 
                            mdopen!(Tag::TableRow),
                        Tag::TableCell =>
                            mdopen!(Tag::TableCell),
                        Tag::Emphasis => 
                            mdopen!(Tag::Emphasis),
                        Tag::Strong => 
                            mdopen!(Tag::Strong),
                        Tag::Strikethrough => 
                            mdopen!(Tag::Strikethrough),
                        Tag::Link(linktype, url, title) =>
                            mdopen!(Tag::Link(linktype, url, title)),
                        Tag::Image(linktype, url, title) =>
                            mdopen!(Tag::Image(linktype, url, title)),
                    },
                Event::End(x) =>
                    match x {
                        Tag::Paragraph =>
                            mdclose!(Tag::Paragraph, *P_META)?,
                        Tag::Heading(level, fragmentid, classes) => {
                            {
                                // Store generated HTML for this
                                // heading in markdownmeta, too,
                                // and add a reference to the html
                                // element in the body.
                                let frame = current_frame!();
                                let bodyslice = frame.body.as_slice();
                                tmp.clear();
                                for node in bodyslice.iter_node(html) {
                                    node.print_plain(&mut tmp, html)?;
                                }
                                anchor_name.clear();
                                text_to_anchor(&tmp, &mut anchor_name);

                                // Append number if necessary to avoid conflicts
                                // (XX should actually do a check like this on the whole
                                // generated page (uh, preserialized parts!))
                                let anchor_name_kstr;
                                'search: loop { // loop bc labels on blocks are unstable
                                    for _ in 0..10 {
                                        if let Some(counter) = anchor_names.get_mut(&*anchor_name) {
                                            *counter += 1;
                                            anchor_name.push_str(&format!("-{}", *counter));
                                        } else {
                                            anchor_name_kstr = KString::from(&anchor_name); 
                                            anchor_names.insert(anchor_name_kstr.clone(), 1);
                                            break 'search;
                                        }
                                    }
                                    warn!("more than 10 *levels* of conflicts trying to find \
                                           unallocated name; leaving it conflicting");
                                    anchor_name_kstr = KString::from(&anchor_name);
                                    break;
                                }

                                frame.atts.push(
                                    // XX Should offer an `attribute`
                                    // method that accepts 2 arguments
                                    // which are ToKString. clone should
                                    // be faster than from_str.
                                    html.attribute(
                                        "id", anchor_name_kstr.as_str())?)?;

                                markdownmeta.push_heading(MarkdownHeading {
                                    level,
                                    header: Some(MarkdownHeader{
                                        html: bodyslice,
                                        anchor_name: anchor_name_kstr
                                    }),
                                    subheadings: Vec::new()
                                });
                            }

                            let meta = elementmeta_from_headinglevel(level);
                            // XX todo: handle fragmentid, classes
                            mdclose!(Tag::Heading(level, fragmentid, classes),
                                     meta)?
                        }
                        Tag::BlockQuote =>
                            mdclose!(Tag::BlockQuote, *BLOCKQUOTE_META)?,
                        Tag::CodeBlock(kind) => 
                        // XX kind -> class="language-xxx", and do highlighting
                            mdclose!(Tag::CodeBlock(kind), *PRE_META)?,
                            
                        Tag::List(firstitemnum) =>
                            mdclose!(
                                Tag::List(firstitemnum),
                                if firstitemnum.is_some() {
                                    *OL_META
                                } else {
                                    *UL_META
                                })?,
                        Tag::Item =>
                            mdclose!(Tag::Item, *LI_META)?,
                        Tag::FootnoteDefinition(label) => {
                            // A footnote definition. The value contained is the footnote's
                            // label by which it can be referred to.
                            let frame = context.pop().expect("start before end");
                            if let Some(FootnoteDefinition { text: footnote_text, .. })
                                = markdownmeta.footnotes.get_mut(&*label)
                            {
                                if let Some(_) = footnote_text {
                                    bail!("multiple definitions of a footnote with the \
                                           label {:?}", &*label)
                                } else {
                                    *footnote_text = Some(frame.body.as_slice());
                                    // XX what about atts?
                                }
                            } else {
                                // Definition before first use
                                markdownmeta.footnotes.insert(
                                    KString::from_ref(&*label),
                                    FootnoteDefinition {
                                        reference: None,
                                        text: Some(frame.body.as_slice()),
                                        backreferences: Vec::new(),
                                    });
                            }
                        }
                        Tag::Table(alignments) =>
                            mdclose!(Tag::Table(alignments),
                                     // XX todo: handle alignments
                                     *TABLE_META)?,
                        Tag::TableHead => 
                            mdclose!(Tag::TableHead, *TH_META)?,
                        Tag::TableRow => 
                            mdclose!(Tag::TableRow, *TR_META)?,
                        Tag::TableCell => 
                            mdclose!(Tag::TableCell, *TD_META)?,
                        Tag::Emphasis => 
                            mdclose!(Tag::Emphasis, *EM_META)?,
                        Tag::Strong => 
                            mdclose!(Tag::Strong, *STRONG_META)?,
                        Tag::Strikethrough => 
                            mdclose!(Tag::Strikethrough, *S_META)?,
                        Tag::Link(linktype, url, title) => {
                            let (mut atts, body, outerframe) =
                                mdpop!(
                                    // XX uh, need to clone just to verify. better?
                                    Tag::Link(linktype, url.clone(), title));

                            let elt = match linktype {
                                // Inline link like `[foo](bar)`
                                LinkType::Inline => {
                                    atts.push(
                                        html.attribute("href", kstring_myfrom2(url))?)?;
                                    html.a(atts, body)
                                }
                                // Reference link like `[foo][bar]`
                                LinkType::Reference => {
                                    warn_todo!("LinkType::Reference: \
                                                url, presumably?");
                                    atts.push(
                                        html.attribute("href", kstring_myfrom2(url))?)?;
                                    html.a(atts, body)
                                },
                                // Reference without destination in
                                // the document, but resolved by the
                                // broken_link_callback
                                LinkType::ReferenceUnknown => todo!(),
                                // Collapsed link like `[foo][]`
                                LinkType::Collapsed => todo!(),
                                // Collapsed link without destination
                                // in the document, but resolved by
                                // the broken_link_callback
                                LinkType::CollapsedUnknown => todo!(),
                                // Shortcut link like `[foo]`
                                LinkType::Shortcut => {
                                    warn_todo!("LinkType::Shortcut: need to build \
                                                index and look up");
                                    atts.push(
                                        html.attribute("href", kstring_myfrom2(url))?)?;
                                    html.a(atts, body)
                                },
                                // Shortcut without destination in the
                                // document, but resolved by the
                                // broken_link_callback
                                LinkType::ShortcutUnknown => todo!(),
                                // Autolink like `<http://foo.bar/baz>`
                                LinkType::Autolink =>
                                    html.a([att("href", kstring_myfrom2(url))],
                                           body),
                                // Email address in autolink like `<john@example.org>`
                                LinkType::Email =>
                                    html.a([att("href", email_url(&url))],
                                           body),
                            };
                            outerframe.body.push(elt?)?;
                        }
                        Tag::Image(linktype, url, title) =>
                        // Oh, almost COPYPASTE of Tag::Link
                        {
                            let (mut atts, body, outerframe) =
                                mdpop!(
                                    // XX uh, need to clone just to verify. better?
                                    Tag::Link(linktype, url.clone(), title));
                            let elt = match linktype {
                                LinkType::Inline => {
                                    atts.push(
                                        html.attribute("src", kstring_myfrom2(url))?)?;
                                    html.img(atts, body)
                                }
                                LinkType::Reference => todo!(),
                                LinkType::ReferenceUnknown => todo!(),
                                LinkType::Collapsed => todo!(),
                                LinkType::CollapsedUnknown => todo!(),
                                LinkType::Shortcut => todo!(),
                                LinkType::ShortcutUnknown => todo!(),
                                LinkType::Autolink => todo!(),
                                LinkType::Email => todo!(),
                            };
                            outerframe.body.push(elt?)?;
                        }
                    },
                Event::Text(s) => {
                    let frame = current_frame!();
                    frame.body.push(html.str(&s)?)?;
                }
                Event::Code(s) => {
                    warn!("Event::Code({:?})", &*s);
                    let frame = current_frame!();
                    let elt = html.code(
                        [],
                        [
                            html.str(&s)?
                        ])?;
                    frame.body.push(elt)?;
                }
                Event::Html(s) => {
                    // I don't really want to put it all in here. This
                    // function is horribly long. But working with
                    // closures and hygienic macros in a way to re-use
                    // them, move them outside, is too painful for me
                    // right now, so I go.
                    dt!(&format!("Event::Html({s:?})"));
                    for token in html5gum::Tokenizer::new(&*s).infallible() {
                        match token {
                            Token::StartTag(starttag) => {
                                let name: &str = std::str::from_utf8(
                                    &**starttag.name)?;
                                let meta = METADB.elementmeta.get(name).ok_or_else(
                                    || error_not_an_html5_tag_name(name))?;
                                let mut newframe = new_contextframe!(
                                    ContextTag::Html(meta));
                                for (k, v) in starttag.attributes {
                                    newframe.atts.push(
                                        html.attribute(
                                            kstring(k)?, kstring(v)?)?)?;
                                }
                                if starttag.self_closing || ! meta.has_closing_tag {
                                    let cf = current_frame!();
                                    // XX give context to errors,
                                    // e.g. invalid attribute because,
                                    // where was the element coming
                                    // from? Or utf-8 conversion errors above, too.
                                    cf.body.push(frame_to_element(newframe, meta)?)?;
                                } else {
                                    context.push(newframe);
                                }
                            }
                            Token::EndTag(endtag) => {
                                let name: &str = std::str::from_utf8(
                                    &**endtag.name)?;
                                let meta = METADB.elementmeta.get(name).ok_or_else(
                                    || error_not_an_html5_tag_name(name))?;
                                if meta.has_closing_tag {
                                    let (atts, body, outerframe) =
                                        // XX error context. if only I had
                                        // location info? sigh?
                                        pop!(ContextTag::Html(meta));
                                    // Special HTML tag treatments
                                    if meta == *TITLE_META {
                                        if markdownmeta.title.is_some() {
                                            bail!("multiple <title> elements")
                                        }
                                        markdownmeta.title = Some(body.as_slice());
                                        // XX dropping atts OK?
                                    } else {
                                        outerframe.body.push(
                                            html.new_element(meta,
                                                             atts.as_slice(),
                                                             body.as_slice())?)?;
                                    }
                                } else {
                                    // NOOP, we haven't made a frame for it.
                                }
                            }
                            Token::String(s) => {
                                let frame = current_frame!();
                                frame.body.push(html.kstring(kstring(s)?)?)?;
                            }
                            Token::Comment(_s) => {
                                // This happens only when <!-- and -->
                                // appear in the same markdown event,
                                // i.e. in the same paragraph.  todo:
                                // do something with _s?
                            },
                            Token::Doctype(_) => todo!(),
                            Token::Error(e) =>
                                if s.starts_with("<!--") {
                                    // XX how to check `e` ? Should verify it's "eof-in-comment"
                                    // let newframe = new_contextframe!(
                                    //     ContextTag::HtmlComment);
                                    // context.push(newframe);

                                    // No, slurp up markdown
                                    // events right here until -->
                                    // appears.
                                    while let Some(item) = parser.next() {
                                        match item {
                                            Event::Html(s) =>
                                                if s.starts_with("-->") {
                                                    break
                                                },
                                            _ => ()
                                        }
                                    }
                                } else {
                                    bail!("HTML5 parsing error: {e} for {s:?}")
                                }
                        }
                    }
                }
                Event::FootnoteReference(label) => {
                    // "A reference to a footnote with given label, which may or may
                    // not be defined by an event with a `Tag::FootnoteDefinition`
                    // tag. Definitions and references to them may occur in any
                    // order."
                    let backref = Backref(next_footnote_backreference());
                    let reference =
                        if let Some(fnd) = markdownmeta.footnotes.get_mut(
                            &*label) {
                            let reference =
                                if let Some(reference) = fnd.reference {
                                    reference
                                } else {
                                    let reference = Footnoteref(next_footnote_number());
                                    fnd.reference = Some(reference);
                                    reference
                                };
                            fnd.backreferences.push(backref.clone());
                            reference
                        } else {
                            let reference = Footnoteref(next_footnote_number());
                            markdownmeta.footnotes.insert(
                                KString::from_ref(&*label),
                                FootnoteDefinition {
                                    reference: Some(reference),
                                    text: None,
                                    backreferences: vec![backref.clone()],
                                });
                            reference
                        };

                    let frame = current_frame!();
                    if let Some(i) = frame.last_footnote_reference {
                        if i == frame.body.len() {
                            // Separate the new reference from the
                            // last reference; todo?: ideally the 3
                            // `sup` would be merged.
                            frame.body.push(
                                html.sup(
                                    [],
                                    [html.str(",")?])?)?;
                        }
                    }
                    frame.body.push(
                        html.sup(
                            [att("id", backref.to_kstring(false)),],
                            [html.a(
                                [att("href", reference.to_kstring(true))],
                                [html.string(reference.0.to_string())?])?])?)?;
                    frame.last_footnote_reference = Some(frame.body.len());
                }
                Event::SoftBreak => {
                    // a single \n in the input
                    let frame = current_frame!();
                    frame.body.push(html.str("\n")?)?;
                }
                Event::HardBreak => {
                    // "  \n" in the input
                    let frame = current_frame!();
                    frame.body.push(html.br([], [])?)?;
                }
                Event::Rule => {
                    let frame = current_frame!();
                    frame.body.push(html.hr(
                        [],
                        [])?)?;
                }
                Event::TaskListMarker(checked) => {
                    let frame = current_frame!();
                    let mut atts = html.new_vec();
                    atts.push(html.attribute("type", "checkbox")?)?;
                    atts.push(html.attribute("disabled", "")?)?;
                    if checked {
                        atts.push(html.attribute("checked", "")?)?;
                    }
                    frame.body.push(
                        html.input(
                            atts,
                            [])?)?;
                }
            }
        }
        
        match context.len() {
            0 => bail!("top-level context was dropped -- should be impossible?"),
            1 => (),
            n => bail!("{} non-closed context(s) at end of markdown document: {}",
                       n - 1,
                       context[1..].iter().map(
                           |c| c.tag.to_string())
                       .collect::<Vec<String>>()
                       .join(", "))
        }
        let baseframe = context.pop().unwrap();
        Ok(ProcessedMarkdown {
            html: frame_to_element(baseframe, *DIV_META)?,
            meta: markdownmeta
        })
    }
}
