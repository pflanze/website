use std::pin::Pin;

pub struct StringSplit {
    string: Pin<Box<str>>,
    items: Vec<&'static str>,
}

/// Owns a `str` and the sub-slices into it after a split.
impl StringSplit {
    /// Splits a string 
    pub fn split(string: Box<str>, separator: &str, omit_tailing_entry: bool) -> Self {
        let string = Pin::new(string);
        let stringptr: *const str = &*string;
        let stringstatic: &'static str = unsafe {
            // Safe because we're referencing memory that's in a box,
            // on the heap, in a Pin, and the references we hand out
            // have the same life time as Self. Miri doesn't agree,
            // though, and wants us to box the whole thing again.
            &*stringptr
        };
        let mut items: Vec<&str> = stringstatic.split(separator).collect();
        if omit_tailing_entry {
            if let Some(&"") = items.last() {
                items.pop();
            }
        }
        StringSplit { string, items }
    }
    pub fn string(&self) -> &str {
        &*self.string
    }
    pub fn items(&self) -> &[&str] {
        &*self.items
    }
}
