//! Utilities for HTML contexts

use crate::{att, ASlice, HtmlAllocator, Node};
use anyhow::Result;

use chj_util::nowarn as warn;

fn find_from(s: &str, pos: usize, needle: &str) -> Option<usize> {
    (&s[pos..]).find(needle).map(|p| p + pos)
}

// Characters that are OK in http/https URLs *for auto-detection
// purposes*
fn is_url_character(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || c == ':'
        || c == '/'
        || c == '.'
        || c == '-'
        || c == '%'
        || c == '='
}

// Characters unlikely to be part of an URL *at its end* (but OK in
// the middle)
fn is_character_to_exclude_at_end(c: char) -> bool {
    c == ':' || c == '.' || c == '-' || c == '%' || c == '='
}

// Characters expected to be in front (left) of an URL.
fn is_character_in_front(c: Option<char>) -> bool {
    // word boundary
    if let Some(c) = c {
        !c.is_ascii_alphanumeric()
    } else {
        true
    }
}

/// Convert `text` to an HTML node list usable as HTML Element body,
/// creating links around "http" and "https" URLs contained in `text`.
pub fn autolink(html: &HtmlAllocator, text: &str) -> Result<ASlice<Node>> {
    warn!("autolink for {text:?}");
    let mut nodes = html.new_vec();
    let mut pos_done = 0;
    let mut pos_remainder = 0;
    while let Some(pos) = find_from(text, pos_remainder, "http") {
        warn!("found pos={pos}");
        let pos_rest = pos + 4; // after the 'http'

        let mut backwardsiter = (&text[0..pos]).chars().rev();
        if !is_character_in_front(backwardsiter.next()) {
            warn!("nope 0");
            pos_remainder = pos_rest;
            continue;
        }

        let mut restiter = (&text[pos_rest..]).chars();
        let c0 = restiter.next();
        let c1 = restiter.next();
        let skip_len = match c0 {
            Some('s') => match c1 {
                Some(':') => 2,
                _ => {
                    warn!("nope 1");
                    pos_remainder = pos_rest;
                    continue;
                }
            },
            Some(':') => 1,
            _ => {
                warn!("nope 2");
                pos_remainder = pos_rest;
                continue;
            }
        };
        let pos_rest = pos_rest + skip_len;
        if !(&text[pos_rest..]).starts_with("//") {
            warn!("nope: no // in {:?}", &text[pos_rest..]);
            pos_remainder = pos_rest;
            continue;
        }
        let pos_rest = pos_rest + 2;
        let (one_before_end, end); // in text
        'find_end: loop { // loop bc labels on blocks are unstable
            let mut last_i = 0;
            for (i, c) in (&text[pos_rest..]).char_indices() {
                if !is_url_character(c) {
                    one_before_end = pos_rest + last_i;
                    end = pos_rest + i;
                    break 'find_end;
                }
                last_i = i;
            }
            one_before_end = pos_rest + last_i;
            end = text.len();
            break;
        }

        if one_before_end == end {
            warn!("nope: nothing after //");
            pos_remainder = pos_rest;
            continue;
        }

        let char_before_end = text[one_before_end..]
            .chars()
            .next()
            .expect("char is there because we maintained one_before_end to point there"); // XXX ah, but 0 ?
        let real_end = if is_character_to_exclude_at_end(char_before_end) {
            one_before_end
        } else {
            end
        };
        let url = &text[pos..real_end];

        if pos - pos_done > 0 {
            nodes.push(html.text(&text[pos_done..pos])?)?;
        }
        let link = html.a([att("href", url)], [html.text(url)?])?;
        nodes.push(link)?;
        warn!("pushed node: {}", html.to_html_string(link, false));

        pos_done = real_end;
        pos_remainder = real_end;
    }

    if pos_done < text.len() {
        nodes.push(html.text(&text[pos_done..])?)?;
    }

    Ok(nodes.as_slice())
}

#[cfg(test)]
mod tests {
    use crate::Print;

    use super::*;

    #[test]
    fn t_find_from() {
        assert_eq!(find_from("hello world", 0, "World"), None);
        assert_eq!(find_from("hello world", 0, "world"), Some(6));
        assert_eq!(find_from("hello world", 5, "world"), Some(6));
        assert_eq!(find_from("hello world", 6, "world"), Some(6));
        assert_eq!(find_from("hello world", 7, "world"), None);
        assert_eq!(find_from("hello world in many worlds", 3, "world"), Some(6));
        assert_eq!(
            find_from("hello world in many worlds", 7, "world"),
            Some(20)
        );
    }

    fn t(s: &str) -> String {
        let html = HtmlAllocator::new(1000);
        let slice = autolink(&html, s).unwrap();
        slice.to_html_fragment_string(&html).unwrap()
    }

    #[test]
    fn t_() {
        assert_eq!(t("http:// "), "http:// ");
        assert_eq!(t("http://"), "http://");
        assert_eq!(t(""), "");
        assert_eq!(t("foo"), "foo");
        assert_eq!(t("http"), "http");
        assert_eq!(t("https"), "https");
        assert_eq!(t("http:"), "http:");
        assert_eq!(t("http:/"), "http:/");
        assert_eq!(t("http://foo"), "<a href=\"http://foo\">http://foo</a>");
        assert_eq!(
            t("There's http://foo.com there."),
            "There&#39;s <a href=\"http://foo.com\">http://foo.com</a> there."
        );
        assert_eq!(
            t("There's http://foo.com. Yes."),
            "There&#39;s <a href=\"http://foo.com\">http://foo.com</a>. Yes."
        );
        assert_eq!(
            t("http://foo.com."),
            "<a href=\"http://foo.com\">http://foo.com</a>."
        );
        assert_eq!(t("hmhttp://foo.com."), "hmhttp://foo.com.");
    }
}
