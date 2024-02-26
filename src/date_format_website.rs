//! Adapt to date_format.rs' use of Lang.

//! Going via string slice intermediate to adapt from Language to
//! Lang, falling back to "en" in case it's necessary.

//! (Best compromise between 'type safety' and flexibility?)

use std::time::SystemTime;

use chrono_tz::Europe::Zurich;

use crate::{language::Language,
            date_format::date_format_httplike,
            lang_en_de::Lang};

pub fn date_format_httplike_switzerland<L: Language>(t: SystemTime, lang: L) -> String {
    let langname = lang.as_str();
    date_format_httplike(t, Zurich, Lang::from(langname))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn t_1() {
        let t = SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(1708967013)).unwrap();
        assert_eq!(&*date_format_httplike_switzerland(t, Lang::En),
                   "Mon, 26 Feb 2024 18:03:33 CET");
        assert_eq!(&*date_format_httplike_switzerland(t, Lang::De),
                   "Mo, 26. Feb 2024 18:03:33 CET");
    }

    #[test]
    fn t_2() {
        let t = SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(1714258620)).unwrap();
        assert_eq!(&*date_format_httplike_switzerland(t, Lang::En),
                   "Sun, 28 Apr 2024 00:57:00 CEST");
        assert_eq!(&*date_format_httplike_switzerland(t, Lang::De),
                   "So, 28. Apr 2024 00:57:00 CEST");
    }
    
    #[test]
    fn t_last_cet() {
        let t = SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(1711846799)).unwrap();
        assert_eq!(&*date_format_httplike_switzerland(t, Lang::En),
                   "Sun, 31 Mar 2024 01:59:59 CET");
        assert_eq!(&*date_format_httplike_switzerland(t, Lang::De),
                   "So, 31. Mär 2024 01:59:59 CET");
    }

    #[test]
    fn t_first_cest() {
        let t = SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(1711846800)).unwrap();
        assert_eq!(&*date_format_httplike_switzerland(t, Lang::En),
                   "Sun, 31 Mar 2024 03:00:00 CEST");
        assert_eq!(&*date_format_httplike_switzerland(t, Lang::De),
                   "So, 31. Mär 2024 03:00:00 CEST");
    }

}
