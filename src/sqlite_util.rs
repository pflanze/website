// Weird, Option<Vec<u8>> is not BindableWithIndex

// Aha, ReadableWithIndex for Vec<u8> does exist.
// And bind method exists on Vec<u8> but it's not BindableWithIndex, right?

// pub enum Value {
//     /// Binary data.
//     Binary(Vec<u8>),
//     /// A floating-point number.
//     Float(f64),
//     /// An integer number.
//     Integer(i64),
//     /// A string.
//     String(String),
//     /// A null value.
//     Null,
// }

// implement!(Vec<u8>, Binary);
// implement!(@value Vec<u8>, Binary);
// But those don't implement `bind`

// Anyway, so we hack something together:

use sqlite::BindableWithIndex;

pub fn bind_option_vec_u8<I: sqlite::ParameterIndex>(
    val: &Option<Vec<u8>>, st: &mut sqlite::Statement, idx: I
) -> sqlite::Result<()> {
    // as BLOB
    // Ah, BindableWithIndex *is* implemented for &[u8], oh my.
    val.as_ref().map(|v| v.as_slice()).bind(st, idx)
}
