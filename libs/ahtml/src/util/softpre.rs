use anyhow::Result;

use crate::{AId, Node, HtmlAllocator, ToASlice, att, NBSP};

use super::autolink;


/// Create plain text to HTML that mimicks <pre>, but still allows
/// dynamic line breaks
pub struct SoftPre {
    /// Whether to replace tabs with the given number of non-breaking
    /// spaces.
    pub tabs_to_nbsp: Option<u8>,

    /// Whether to turn http:// and https:// URLs into links.
    pub autolink: bool,

    /// The text is split on this string into lines
    pub line_separator: &'static str
}

impl Default for SoftPre {
    fn default() -> Self {
        Self {
            tabs_to_nbsp: Some(8),
            autolink: true,
            line_separator: "\n",
        }
    }
}

impl SoftPre {
    pub fn format(&self, text: &str, html: &HtmlAllocator) -> Result<AId<Node>> {
        let mut formatted_body = html.new_vec();
        for line in text.split(self.line_separator) {
            let mut formatted_line =
                if self.autolink {
                    autolink(html, line)?
                } else {
                    html.text(line)?.to_aslice(html)?
                };

            if let Some(n) = self.tabs_to_nbsp {
                let mut items = html.new_vec();
                for id in formatted_line.iter_aid(html) {
                    match html.get_node(id).expect("todo: when can this fail?") {
                        Node::String(s) => {
                            let mut s2 = String::new();
                            for c in s.chars() {
                                if c == '\t' {
                                    for _ in 0..n {
                                        s2.push_str(NBSP)
                                    }
                                } else {
                                    s2.push(c)
                                }
                            }
                            items.push(html.text(s2)?)?;
                        }
                        _ => items.push(id)?
                    }
                }
                formatted_line = items.as_slice();
            }

            // (Future: also map over the items and replace
            // multiple-space sections in all the text segments with
            // space/nbsp alterations? Difficulty is those ending up
            // on the next line for wrapped lines, though?)

            formatted_body.append(formatted_line)?;
            formatted_body.push(html.br([],[])?)?;
        }
        html.div([att("class", "soft_pre")], formatted_body)
    }
}

#[cfg(test)]
mod tests {
    use crate::Print;

    use super::*;

    #[test]
    fn t_softpre() -> Result<()> {
        let softpre = SoftPre::default();
        let html = HtmlAllocator::new(1000, std::sync::Arc::new(""));
        let t = |s| -> String {
            softpre.format(s, &html).unwrap().to_html_fragment_string(&html).unwrap()
        };
        assert_eq!(t("foo bar"), "<div class=\"soft_pre\">foo bar<br></div>");
        assert_eq!(t("foo bar\n\tbaz"), "<div class=\"soft_pre\">foo bar<br>\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}baz<br></div>");
        Ok(())
    }
}

