use anyhow::Result;
use kstring::KString;

use ahtml::{HtmlAllocator, AId, Node, TryCollectBody, att};
use auri::ppath::PPath;

use crate::{acontext::AContext, language::Language};

pub trait ToHtml {
    fn to_html<L: Language>(
        &self, html: &HtmlAllocator, context: &AContext<L>
    ) -> Result<AId<Node>>;
}


pub enum SubEntries {
    Static(&'static [NavEntry]),
    MdDir(&'static str), // Path
}
impl ToHtml for SubEntries {
    fn to_html<L: Language>(
        &self, _html: &HtmlAllocator, _contex: &AContext<L>
    ) -> Result<AId<Node>> {
        todo!()
    }
}

pub struct NavEntry {
    pub name: &'static str,
    pub path: &'static str,
    pub subentries: SubEntries
}
impl ToHtml for NavEntry {
    fn to_html<L: Language>(
        &self, html: &HtmlAllocator, context: &AContext<L>
    ) -> Result<AId<Node>> {
        let name = html.staticstr(self.name)?;
        html.li(
            [],
            [
                if context.path().same_document_as_path_str(self.path) {
                    name
                } else {
                    let rel = self.ppath().sub(context.path())?;
                    html.a(
                        [att("href", rel.to_string())],
                        [name])?
                }
            ])
    }
}
impl NavEntry {
    fn ppath(&self) -> PPath<KString> {
        PPath::from_str(self.path)
    }
}

pub struct Nav<'t>(pub &'t [NavEntry]);

impl<'t> ToHtml for Nav<'t> {
    fn to_html<L: Language>(
        &self, html: &HtmlAllocator, context: &AContext<L>
    ) -> Result<AId<Node>> {
        Ok(html.ul(
            [att("class", "nav")],
            self.0.iter().map(|naventry| naventry.to_html(html, context))
                .try_collect_body(html)?)?)
    }
}
