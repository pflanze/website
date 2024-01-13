//! Debug trace

// Sadly there's no __func__ or __FUNCTION__ equivalent in Rust.

use std::cell::Cell;

thread_local! {
    // Cell not working here in 1.63.0, thus go with RefCell anyway
    static LEVEL: Cell<u32> = Cell::new(0);
}

const INDENT: &str = "                                                                                                                                                                                                        ";

fn indent(n: u32) -> &'static str {
    &INDENT[0..(n as usize)]
}

pub struct DtGuard {
    pub string: String
}

impl Drop for DtGuard {
    fn drop(&mut self) {
        // leave
        let l: u32 = LEVEL.with(|c: &Cell<u32>| {
            let new = c.get() - 1;
            c.set(new);
            new
        });
        eprintln!("{}{}[90m<- ({}){}[30m",
                  // ^ 37 is too bright; 30 assuming black is default
                  indent(l),
                  27 as char, // \033
                  self.string,
                  27 as char);
    }
}

pub fn enter(s: &str) {
    let l: u32 = LEVEL.with(|c: &std::cell::Cell<u32>| {
        let old = c.get();
        c.set(old + 1);
        old
    });
    eprintln!("{}-> ({})",
              indent(l),
              s);
}

#[macro_export]
macro_rules! dt {
    ($namestr:expr $(,$arg:expr)*) => {
        // let namestr = stringify!($name);
        let mut guard = dt::DtGuard {
            string: String::new()
        };
        guard.string.push_str($namestr);
        $(
            guard.string.push_str(&format!(" {:?}", $arg));
        )*
        dt::enter(&guard.string);
    }
}

#[macro_export]
macro_rules! nodt {
    ($namestr:expr $(,$arg:expr)*) => {
    }
}

