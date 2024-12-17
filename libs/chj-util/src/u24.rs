//! A 24-bit unsigned integer type that only takes up 24 bits of space
//! (unlike `u24` in the `ux` crate which takes up "as much space as
//! the smallest integer type that can contain [it]" and hence 32
//! bits?)

pub const U24MAX: u32 = 1 << 24 - 1;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct U24([u8; 3]);

impl U24 {
    pub fn new(n: u32) -> U24 {
        assert!(n <= U24MAX);
        U24([
            (n & 255) as u8,
            ((n >> 8) & 255) as u8,
            (n >> 16) as u8
        ])
    }
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::*;

    #[allow(unused)]
    struct T1 {
        a: U24,
    }
    #[allow(unused)]
    struct T2 {
        a: U24,
        b: u8
    }
    #[allow(unused)]
    struct T3 {
        a: bool,
        b: U24,
    }
    
    #[test]
    fn t_u24() {
        assert_eq!(size_of::<U24>(), 3);
    }
    #[test]
    fn t_t1() {
        assert_eq!(size_of::<T1>(), 3);
    }
    #[test]
    fn t_t2() {
        assert_eq!(size_of::<T2>(), 4);
    }
    #[test]
    fn t_t3() {
        assert_eq!(size_of::<T3>(), 4);
    }
}
