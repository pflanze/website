//! Implementation of `Language` for English and German (in that
//! priority order).

use crate::language::Language;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Lang {
    En,
    De,
}

const MEMBERS: &[Lang] = &[Lang::En, Lang::De];
// stupid logical copy just for the sake of having it in the binary:
const STRS: &[&str] = &["en", "de"];

impl Language for Lang {
    // XX use some parse trait instead ?

    fn maybe_from(s: &str) -> Option<Self> {
        match dbg!(s) {
            "en" => Some(Lang::En),
            "de" => Some(Lang::De),
            _ => None
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::De => "de",
        }
    }

    fn members() -> &'static [Self] {
        MEMBERS
    }

    fn strs() -> &'static [&'static str] {
        STRS
    }
}

impl Default for Lang {
    fn default() -> Self {
        MEMBERS[0]
    }
}

impl From<&str> for Lang {
    fn from(s: &str) -> Self {
        Lang::maybe_from_start(s).unwrap_or_default()
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
        assert_eq!(Lang::default(), Lang::En);
    }
}
