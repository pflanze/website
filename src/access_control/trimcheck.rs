//! Sanitize user-provided inputs

use std::ops::RangeInclusive;

use crate::def_boxed_thiserror;

// number of string characters
const LENRANGE_USERNAME: RangeInclusive<u32> = 2..=20;
const LENRANGE_GROUPNAME: RangeInclusive<u32> = 1..=20;
const LENRANGE_PASSWORD: RangeInclusive<u32> = 8..=200;
const LENRANGE_EMAIL: RangeInclusive<u32> = 3..=120;

def_boxed_thiserror!(InputCheckFailure, pub enum InputCheckFailureKind {
    #[error("{0} is too long, must be {1:?} characters")]
    TooLong(&'static str, RangeInclusive<u32>),
    #[error("{0} is too short, must be {1:?} characters")]
    TooShort(&'static str, RangeInclusive<u32>),
    #[error("{0} contains the \\0 character")]
    ContainsNull(&'static str),
    #[error("{0} is missing the '@' character")]
    MissingAt(&'static str),
});

fn trimcheck_<'s>(
    fieldname: &'static str,
    len_range: RangeInclusive<u32>,
    s: &'s str
) -> Result<&'s str, InputCheckFailure>
{
    if s.contains('\0') {
        Err(InputCheckFailureKind::ContainsNull(fieldname))?
    }
    let s = s.trim();
    let len = s.len();
    if len < *len_range.start() as usize {
        Err(InputCheckFailureKind::TooShort(fieldname, len_range))?
    } else if len >= *len_range.end() as usize {
        Err(InputCheckFailureKind::TooLong(fieldname, len_range))?
    } else {
        Ok(s)
    }
}

pub fn trimcheck_username(s: &str) -> Result<&str, InputCheckFailure> {
    trimcheck_("username", LENRANGE_USERNAME, s)
}

pub fn trimcheck_groupname(s: &str) -> Result<&str, InputCheckFailure> {
    trimcheck_("groupname", LENRANGE_GROUPNAME, s)
}

pub fn trimcheck_password(s: &str) -> Result<&str, InputCheckFailure> {
    trimcheck_("password", LENRANGE_PASSWORD, s)
}

pub fn trimcheck_email(s: &str) -> Result<Option<&str>, InputCheckFailure> {
    let s = s.trim();
    if s.is_empty() {
        Ok(None)
    } else {
        if s.contains('@') {
            Ok(Some(trimcheck_("email", LENRANGE_EMAIL, s)?))
        } else {
            Err(InputCheckFailureKind::MissingAt("email"))?
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::*;

    #[test]
    fn t_result_size() {
        if size_of::<usize>() == 8 {
            // slice and tag
            // XX why different results? assert_eq!(size_of::<Result<&str, bool>>(), 3 * 8);

            // 2 str, <2 range, tag in range
            assert_eq!(size_of::<InputCheckFailureKind>(), 4 * 8);
            // 1 more for tag
            // XX assert_eq!(size_of::<Result<&str, InputCheckFailureKind>>(), 5 * 8);

            // XX assert_eq!(size_of::<InputCheckFailure>(), 1 * 8);
            // XX assert_eq!(size_of::<Result<&str, InputCheckFailure>>(), 3 * 8);
        }
    }
}
