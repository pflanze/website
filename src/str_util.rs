// copies from cj-hours-parser

/// Drop the first `n` characters. If `s` has fewer than `n`
/// characters, returns the empty string.
pub fn str_drop(s: &str, n: usize) -> &str {
    if n == 0 {
        s
    } else {
        let mut chars = s.chars(); chars.nth(n-1); chars.as_str()
    }
}

#[test]
fn t_str_drop() {
    assert_eq!(str_drop(" Hey there", 0), " Hey there");
    assert_eq!(str_drop(" Hey there", 1), "Hey there");
    assert_eq!(str_drop(" Hey there", 4), " there");
    assert_eq!(str_drop("Hä lü", 2), " lü");
    assert_eq!(str_drop("Hä lü", 5), "");
    assert_eq!(str_drop("Hello", 55), "");
}


/// Take `n` characters if available, fewer if reaching EOS before that
/// point. Returns true iff (at least) `n` characters were available.
pub fn str_take(s: &str, n: usize) -> (&str, bool) {
    let mut ci = 0;
    for (i, _) in s.char_indices() {
        if ci == n {
            return (&s[0..i], true)
        }
        ci += 1;
    }
    (s, ci == n)
}

