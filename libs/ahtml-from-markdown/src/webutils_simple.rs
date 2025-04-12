

// Use CowStr ?
pub fn email_url(s: &str) -> String {
    if s.starts_with("mailto:") {
        s.into()
    } else if s.starts_with("https:") || s.starts_with("http:") {
        // XX warn!("using a non-email URL where an email address was expected: {s:?}");
        s.into()
    } else {
        // hope all is well !
        format!("mailto:{s}")
    }
}
