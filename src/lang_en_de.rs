//! Implementation of `Language` for English and German (in that
//! priority order).

use std::str::FromStr; // ::from_str()

use strum::VariantNames; // ::VARIANTS
use strum::IntoEnumIterator; // ::iter()

use chj_util::warn;

use crate::language::Language;

#[derive(Debug, PartialEq, Eq, Clone, Copy,
         strum_macros::EnumVariantNames, // ::VARIANTS
         strum::IntoStaticStr, // .into() &str
         strum_macros::EnumIter, // ::iter()
         strum_macros::EnumString, // ::from_str( )
)]
#[strum(serialize_all = "lowercase")]
pub enum Lang {
    En,
    De,
}

impl Language for Lang {
    type MemberIter = LangIter;

    fn maybe_from(s: &str) -> Option<Self> {
        Self::from_str(s).ok()
    }

    fn as_str(self) -> &'static str {
        self.into()
    }

    fn members() -> Self::MemberIter {
        Self::iter()
    }

    fn strs() -> &'static [&'static str] {
        &Self::VARIANTS
    }
}

impl Default for Lang {
    fn default() -> Self {
        Self::iter().next().expect("there is at least one Lang member")
    }
}

impl From<&str> for Lang {
    fn from(s: &str) -> Self {
        Lang::maybe_from_start(s).unwrap_or_default()
    }
}

impl Lang {
    pub fn verbose_from(s: &str) -> Lang {
        Lang::maybe_from_start(s).unwrap_or_else(
            || {
                let l = Lang::default();
                warn!("unhandled language {s:?}, falling back to {:?}",
                      l.as_str());
                l
            })
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
        assert_eq!(Lang::maybe_from_start("def"), Some(Lang::De));
        assert_eq!(Lang::maybe_from_start("dfe"), None);
        assert_eq!(Lang::maybe_from("d"), None);
        assert_eq!(Lang::default(), Lang::En);
    }

    #[test]
    fn t_strs() {
        assert_eq!(Lang::strs(), ["en", "de"]);
    }

    #[test]
    fn t_to() {
        assert_eq!(Lang::De.as_str(), "de");
    }
}
