use pct_str::{PctString, URIReserved, InvalidPctString, PctStr};

pub fn url_encode(s: &str) -> String {
    let p = PctString::encode(s.chars(), URIReserved);
    p.to_string()
}

// Don't want to return InvalidPctString as error value because then:
// 1. dependency on pct_str,
// 2. worse, InvalidPctString would contain &str and that would be
//    embedded in anyhow::Result down the line and that leads to
//    <`request` escapes the function body>.
// Thus make our own that owns the string.

#[derive(Debug, thiserror::Error)]
#[error("url decoding error: {0}")]
pub struct UrlDecodingError(Box<String>);

impl From<InvalidPctString<&str>> for UrlDecodingError {
    fn from(e: InvalidPctString<&str>) -> Self {
        Self(Box::new(format!("{}", e)))
    }
}

pub fn url_decode(s: &str) -> Result<String, UrlDecodingError> {
    let p = PctStr::new(s)?;
    Ok(p.decode())
}

