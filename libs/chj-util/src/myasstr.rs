use kstring::KString;


pub trait MyAsStr {
    fn my_as_str<'t>(&'t self) -> &'t str;
}

impl MyAsStr for KString {
    fn my_as_str(&self) -> &str {
        self.as_str()
    }
}

impl MyAsStr for &KString {
    fn my_as_str(&self) -> &str {
        self.as_str()
    }
}

impl MyAsStr for str {
    fn my_as_str(&self) -> &str {
        self
    }
}

// string literals
impl MyAsStr for &str {
    fn my_as_str(&self) -> &str {
        *self
    }
}

impl MyAsStr for &&str {
    fn my_as_str(&self) -> &str {
        **self
    }
}

impl MyAsStr for String {
    fn my_as_str(&self) -> &str {
        self.as_str()
    }
}

impl MyAsStr for &String {
    fn my_as_str(&self) -> &str {
        self.as_str()
    }
}

