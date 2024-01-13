use anyhow::Result;

use crate::{ahtml::{Node, ASlice, Allocator, AId, Flat, att},
            markdown::{StylingInterface, Footnoteref, Backref, StylingContextInterface},
            util::alphanumber};

// ------------------------------------------------------------------

/// Footnotes in the style of Wikipedia (backreferences after the
/// number instead of after the footnote). Although it currently
/// doesn't use the same markup and needs tweaking.
pub struct WikipediaStyle {}
pub struct WikipediaStyleContext<'c> {
    config: &'c WikipediaStyle,
    spacer: AId<Node>,
    uparrow: AId<Node>,
}

impl StylingInterface for WikipediaStyle {
    fn new_context<'c>(
        &'c self,
        html: &Allocator,
    ) -> Result<Box<dyn StylingContextInterface<'c> + 'c>> {
        Ok(Box::new(WikipediaStyleContext {
            config: self,
            spacer: html.str(" ")?,
            uparrow: html.str("^")?,
        }))
    }
}

impl<'c> StylingContextInterface<'c> for WikipediaStyleContext<'c> {
    fn format_footnote_definition(
        &self,
        html: &Allocator,
        reference: &Footnoteref,
        backreferences: &[Backref],
        clean_slice: &ASlice<Node>,
    ) -> Result<Flat<Node>> {

        let mut refvec = html.new_vec();
        refvec.push(html.string(reference.0.to_string())?)?;
        refvec.push(self.spacer)?;
        match backreferences.len() {
            0 => {},
            1 => {
                let backref = backreferences.first().unwrap();
                refvec.push(
                    html.a(
                        [att("href", backref.to_kstring(true))],
                        [
                            self.uparrow
                        ])?)?;
            }
            _ => {
                refvec.push(self.uparrow)?;
                for (i, backref) in backreferences.iter().enumerate() {
                    refvec.push(self.spacer)?;
                    refvec.push(
                        html.a(
                            [att("href", backref.to_kstring(true))],
                            [
                                html.string(alphanumber(i as u32))?,
                            ])?)?;
                }
            }
        }
        Ok(Flat::Two(
            html.dt(
                [att("class", "footnote_reference"),
                 att("id", reference.to_kstring(false)) ],
                refvec.as_slice())?,
            html.dd([], clean_slice)?))
    }

    fn format_footnotes(
        &self,
        body: ASlice<Node>,
        html: &Allocator,
    ) -> Result<AId<Node>> {
        html.dl(
            [att("class", "footnotes")],
            body)
    }
}

// ------------------------------------------------------------------

/// Footnotes in the style typically used on blogs (backreferences
/// after the footnote). Markup seems fine.
pub struct BlogStyle {
}
pub struct BlogStyleContext<'c> {
    config: &'c BlogStyle,
    spacer: AId<Node>,
    uparrow: AId<Node>,
}

impl StylingInterface for BlogStyle {
    fn new_context<'c>(
        &'c self,
        html: &Allocator,
    ) -> Result<Box<dyn StylingContextInterface<'c> + 'c>> {
        Ok(Box::new(BlogStyleContext {
            config: self,
            spacer: html.str(" ")?,
            uparrow: html.str("↩")?,
        }))
    }
}

impl<'c> StylingContextInterface<'c> for BlogStyleContext<'c> {
    fn format_footnote_definition(
        &self,
        html: &Allocator,
        reference: &Footnoteref,
        backreferences: &[Backref],
        clean_slice: &ASlice<Node>,
    ) -> Result<Flat<Node>> {

        let mut refvec = html.new_vec();
        refvec.extend_from_slice(clean_slice, html)?;
        refvec.push(self.spacer)?;
        match backreferences.len() {
            0 => {}, // can this ever happen or checked for earlier?
            1 => {
                let backref = backreferences.first().unwrap();
                refvec.push(
                    html.a(
                        [att("href", backref.to_kstring(true))],
                        [
                            self.uparrow
                        ])?)?;
            }
            _ => {
                refvec.push(self.uparrow)?;
                for (i, backref) in backreferences.iter().enumerate() {
                    refvec.push(self.spacer)?;
                    refvec.push(
                        html.a(
                            [att("href", backref.to_kstring(true))],
                            [
                                html.string(alphanumber(i as u32))?,
                            ])?)?;
                }
            }
        }
        Ok(Flat::One(
            html.li(
                [att("class", "footnote_definition"),
                 att("id", reference.to_kstring(false)) ],
                refvec.as_slice())?))
    }

    fn format_footnotes(
        &self,
        body: ASlice<Node>,
        html: &Allocator,
    ) -> Result<AId<Node>> {
        html.ol(
            [att("class", "footnotes")],
            body)
    }
}
