use std::fmt::Write;

/// A 12 character (6 entropy bytes) long hex string useful to tag
/// e.g. error messages for identification.
pub fn randomidstring() -> Result<String, getrandom::Error> {
    let mut buf = [0u8; 6];
    getrandom::getrandom(&mut buf)?;
    let mut s = String::new();
    for byte in buf {
        write!(&mut s, "{:02X}", byte).unwrap();
    }
    Ok(s)
}
