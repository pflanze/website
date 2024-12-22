use std::ops::RangeBounds;

fn usize_range_len<R>(range: &R, end_excl: usize) -> usize
where
    R: RangeBounds<usize>
{
    let start = match range.start_bound() {
        std::ops::Bound::Included(a) => *a,
        std::ops::Bound::Excluded(_) => unreachable!(
            "right? a..=b but there's no a=..b; \
             could one construct this programmatically, though?"),
        std::ops::Bound::Unbounded => 0,
    };
    match range.end_bound() {
        std::ops::Bound::Included(e_incl) => {
            // (e_incl - start) + 1,
            e_incl.checked_sub(start).expect("end must be >= start")
                .checked_add(1).expect("range end must be < usize::max()")
        }
        std::ops::Bound::Excluded(e_excl) => {
            // e_excl - start,
            e_excl.checked_sub(start).expect("end must be >= start")
        }
        std::ops::Bound::Unbounded => {
            // end_excl - start,
            end_excl.checked_sub(start).expect("end must be >= start")
        }
    }
}

#[cfg(test)]
fn identity<T>(v: T) -> T {
    v
}
    
#[cfg(test)]
#[test]
fn t_usize_range_len() {
    assert_eq!(usize_range_len(&identity(1..4), 10), 3);
    assert_eq!(usize_range_len(&identity(1..=4), 10), 4);
    assert_eq!(usize_range_len(&identity(1..), 10), 9);
    assert_eq!(usize_range_len(&identity(..), 10), 10);
    assert_eq!(usize_range_len(&identity(..4), 10), 4);
    assert_eq!(usize_range_len(&identity(..=4), 10), 5);
    assert_eq!(usize_range_len(&identity(4..4), 10), 0);
    assert_eq!(usize_range_len(&identity(4..=4), 10), 1);
    // should it catch that? just let later on
    assert_eq!(usize_range_len(&identity(..4), 2), 4);
}
#[test]
#[should_panic]
fn t_usize_range_len_invalid_start() {
    usize_range_len(&identity(5..4), 2);
}
#[test]
#[should_panic]
fn t_usize_range_len_invalid_start_2() {
    usize_range_len(&identity(5..=4), 2);
}
#[test]
#[should_panic]
fn t_usize_range_len_invalid_start_indirect() {
    usize_range_len(&identity(5..), 2);
}

pub trait MoreVec<T> {
    /// See docs on `push_within_capacity` in std, which is nightly-only
    fn push_within_capacity_(&mut self, value: T) -> Result<(), T>;

    /// Like `extend_from_within` but fails if the extension cannot be
    /// done within capacity.
    fn extend_from_within_within_capacity<R>(&mut self, src: R) -> Result<(), ()>
    where
        R: RangeBounds<usize>,
        T: Clone;
}

impl<T> MoreVec<T> for Vec<T> {
    fn push_within_capacity_(&mut self, value: T) -> Result<(), T> {
        if self.len() < self.capacity() {
            self.push(value);
            Ok(())
        } else {
            Err(value)
        }
    }

    fn extend_from_within_within_capacity<R>(&mut self, src: R) -> Result<(), ()>
    where
        R: RangeBounds<usize>,
        T: Clone
    {
        let len = self.len();
        let additional_len = usize_range_len(&src, len);
        let newlen = len + additional_len;
        if newlen > self.capacity() {
            return Err(())
        }
        self.extend_from_within(src);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn t_within_capacity() {
    let mut v = Vec::with_capacity(5);
    v.push_within_capacity_(9).unwrap();
    v.push_within_capacity_(7).unwrap();
    v.push_within_capacity_(1).unwrap();

    assert!(v.extend_from_within_within_capacity(0..3).is_err());
    assert!(v.extend_from_within_within_capacity(1..3).is_ok());
    assert_eq!(v.as_slice(), &[9, 7, 1, 7, 1]);
}

