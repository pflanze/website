use std::time::SystemTime;

use httpdate::fmt_http_date;
use anyhow::Result;
use chrono::Utc;
use kstring::KString;

use crate::{webparts::LayoutInterface,
            arequest::ARequest,
            ahtml::{HtmlAllocator, AId, Node, Flat, ToASlice, att},
            nav::{Nav, ToHtml},
            time_util::LocalYear,
            warn,
            language::Language,
            alist::AList,
            ppath::PPath};

fn year_range(from: i32, to: i32) -> String {
    if from == to {
        from.to_string()
    } else {
        format!("{}–{}", from, to)
    }
}

pub struct WebsiteLayout<L: Language + 'static> {
    pub site_name: &'static str,
    pub copyright_owner: &'static str,
    pub nav: &'static [(L, Nav<'static>)],
    pub header_contents: Box<dyn Fn(&HtmlAllocator) -> Result<Flat<Node>> + Send + Sync>,
    pub sibling_from_path: Box<dyn Fn(&PPath<KString>) -> Option<String> + Send + Sync>,
}

impl<L: Language> LayoutInterface<L> for WebsiteLayout<L> {
    fn page(
        &self,
        request: &ARequest<L>,
        html: &HtmlAllocator,
        // Can't be preserialized HTML, must be string node:
        head_title: Option<AId<Node>>,
        // Same contents as head_title, but may be preserialized HTML;
        // must not contain wrapper element like <h1>:
        title: Option<AId<Node>>,
        breadcrumb: Option<AId<Node>>,
        toc: Option<AId<Node>>,
        lead: Option<AId<Node>>,
        main: AId<Node>,
        footnotes: Option<AId<Node>>,
        last_modified: Option<SystemTime>,
    ) -> Result<AId<Node>>
    {
        let tocbox =
            if let Some(toc) = toc {
                html.div([att("id", "toc_container")],
                         [
                             html.p([att("class", "toc_title")],
                                    [html.staticstr("Contents")?])?,
                             toc
                         ])?
            } else {
                html.span([att("class", "no_toc")],[])?
            };

        let breadcrumb =
            if let Some(breadcrumb) = breadcrumb {
                breadcrumb
            } else {
                html.div([att("class", "no_breadcrumb")], [])?
            };

        let lang: L = request.lang();
        let nav_html =
            if let Some(nav) = AList(self.nav).get(&lang) {
                nav.to_html(&html, request)?
            } else {
                html.empty_node()?
            };
        let langswitcher_html = {
            let mut items = html.new_vec_with_capacity(L::strs().len() as u32)?;
            for l in L::members() {
                let s = html.string(l.as_str().to_uppercase())?;
                let path = request.path();
                let sibling_url = (self.sibling_from_path)(path); // Hack
                items.push(html.li(
                    [],
                    [
                        if *l == lang {
                            s
                        } else {
                            if let Some(sibling_url) = sibling_url {
                                html.a(
                                    [att("href", sibling_url)],
                                    [s])?
                            } else {
                                warn!("missing sibling page for {path:?} for lang {:?}",
                                      lang.as_str());
                                s
                            }
                        }
                    ])?)?
            }
            html.ul([att("class", "langs")],
                    items.as_slice())?
        };
        
        html.html(
            [],
            [
                html.head(
                    [],
                    [
                        html.link(
                            [att("rel", "stylesheet"),
                             att("href", "/static/main.css")],
                            [])?,
                        html.title(
                            [],
                            if let Some(head_title) = head_title {
                                let head_title_string = html.to_plain_string(head_title)?;
                                Flat::Two(
                                    html.to_plain_string_aid(head_title)?,
                                    // Do not show the title if it's
                                    // also the site name
                                    if &head_title_string == self.site_name {
                                        html.empty_node()?
                                    } else {
                                        html.string(format!(" | {}",
                                                            self.site_name))?
                                    }
                                )
                            } else {
                                Flat::One(
                                    html.staticstr(self.site_name)?
                                )
                            })?,
                    ])?,
                html.body(
                    [],
                    [
                        html.div(
                            [att("class", "wrapper")],
                            [
                                // Header
                                html.div(
                                    [att("class", "header")],
                                    (self.header_contents)(html)?.to_aslice(html)?)?,
                                // Nav
                                html.div(
                                    [att("class", "navigation")],
                                    [
                                        html.div(
                                            [att("class", "navrow")],
                                            [
                                                nav_html,
                                                langswitcher_html
                                            ])?,
                                        breadcrumb,
                                    ])?,
                                // Document
                                if let Some(title) = title {
                                    html.h1(
                                        [],
                                        [title])?
                                } else {
                                    html.empty_node()?
                                },
                                if let Some(lead) = lead {
                                    html.div([], [
                                        lead,
                                        tocbox])?
                                } else {
                                    tocbox
                                },
                                html.div(
                                    [att("class", "page-content")],
                                    [main])?,
                                if let Some(footnotes) = footnotes {
                                    html.div(
                                        [],
                                        [
                                            html.hr([att("class", "hr_footnotes")], [])?,
                                            footnotes,
                                        ])?
                                } else {
                                    html.div([att("class", "no_footnotes")],[])?
                                },
                                // Footer
                                html.div(
                                    [att("class", "footer")],
                                    [
                                        if let Some(last_modified) = last_modified {
                                            html.div(
                                                [att("class", "last_modified")],
                                                [html.string(
                                                    format!("Last modified {}",
                                                            fmt_http_date(last_modified)))?])?
                                        } else {
                                            html.empty_node()?
                                        },
                                        html.div(
                                            [att("class", "last_modified")],
                                            [html.string(
                                                format!("Copyright © {} {}",
                                                        year_range(
                                                            2023,
                                                            request.now().local_year(Utc)),
                                                        self.copyright_owner))?])?,
                                    ])?,
                            ])?,
                    ])?
            ])
    }

    fn blog_index_title(
        &self,
        subpath_segments: Option<&[KString]> // path segments if below main page
    ) -> String {
        let title = "Articles"; // "Blog posts" -- XX i18n
        if let Some(segments) = subpath_segments {
            let in_or_on = match segments.len() {
                1 => "in",
                2 => "in",
                3 => "on",
                _ => {
                    warn!("unexpected number of segments");
                    ""
                }, 
            };
            format!("{title} {in_or_on} {}", segments.join("/"))
        } else {
            title.into()
        }
    }
}

