use crate::str_util::str_take;


#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Lang {
    En,
    De,
}

impl Lang {
    // XX use some parse trait instead ?

    pub fn maybe_from(s: &str) -> Option<Self> {
        match dbg!(s) {
            "en" => Some(Lang::En),
            "de" => Some(Lang::De),
            _ => None
        }
    }

    pub fn maybe_from_start(s: &str) -> Option<Self> {
        let (start, ok) = str_take(s, 2);
        if ! ok { return None }
        Self::maybe_from(start)
    }

    pub fn strs() -> &'static [&'static str] {
        &["en", "de"]
    }
}

impl Default for Lang {
    fn default() -> Self {
        Lang::En
    }
}

impl From<&str> for Lang {
    fn from(s: &str) -> Self {
        Lang::maybe_from(s).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_from() {
        assert_eq!(Lang::from("de"), Lang::De);
        assert_eq!(Lang::from("de_CH"), Lang::De);
        assert_eq!(Lang::from("de-CH"), Lang::De);
        assert_eq!(Lang::from("dee"), Lang::De);
        assert_eq!(Lang::from("dfe"), Lang::En);
        assert_eq!(Lang::maybe_from("dfe"), None);
        assert_eq!(Lang::maybe_from("d"), None);
    }
}
