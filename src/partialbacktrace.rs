//! Wrapper around the `backtrace` crate to show only part of the
//! stack frames (skip some at the beginning and end).

use std::fmt::Write;

use backtrace::Backtrace;


pub struct PartialBacktrace {
    bt: Backtrace
}

// Cut away last part from e.g.
//  "website::ahtml::HtmlAllocator::new_element::h63d71c1114df562b"
fn cut_hex(mut s: String) -> String {
    let err = |s, _msg| -> String {
        // warn!("could not cut end of {s:?}: {msg}");
        // Happens for "__GI___clone3", "start_thread"
        s
    };
    let mut cs = s.char_indices().rev();
    while let Some((_, c)) = cs.next() {
        if ! c.is_ascii_hexdigit() {
            if c != 'h' { return err(s, "expecting 'h' left of hex digits") }
            if let Some((_, c)) = cs.next() {
                if c != ':' { return err(s, "expecting ':' left of 'h'") }
                if let Some((pos, c)) = cs.next() {
                    if c != ':' { return err(s, "expecting ':' left of 'h'") }
                    s.truncate(pos);
                    return s
                } else {
                    return err(s, "premature end left of ':'")
                }
            } else {
                return err(s, "expecting :: left of 'h'")
            }
        }
    }
    return err(s, "string ends early left of hex digits")
}

impl PartialBacktrace {
    pub fn new() -> Self {
        PartialBacktrace { bt: Backtrace::new() }
    }

    /// Show the stack frames after the first `skip` ones, until
    /// reaching one (excluding it) that refers to a file with a path
    /// that ends in `end_file`.
    pub fn part_to_string(&self, skip: usize, end_file: &str) -> String {
        let mut bt_str = String::new();
        let frames = &self.bt.frames()[skip..];
        let mut frameno = 0; // starts counting after the skipped area
        'outer: for frame in frames.iter() {
            let mut subframeno = 0;
            for sym in frame.symbols() {
                // Have to reimplement everything as Backtrace' frames
                // don't have the formatting code, only Backtrace as a
                // whole has.
                if let Some(path) = sym.filename() {
                    let p = path.to_string_lossy();
                    if p.ends_with(end_file) {
                        break 'outer;
                    }
                    let name = sym.name().map(|s| cut_hex(s.to_string()))
                        .unwrap_or_else(|| " XX missing name ".into());
                    if subframeno == 0 {
                        write!(&mut bt_str, "{frameno:4}").unwrap();
                    } else {
                        bt_str.push_str("      ");
                    }
                    let indent_at = "             at ";
                    write!(&mut bt_str, ": {name}\n\
                                         {indent_at}{p}").unwrap();
                    if let Some(line) = sym.lineno() {
                        write!(&mut bt_str, ":{line}").unwrap();
                        if let Some(col) = sym.colno() {
                            write!(&mut bt_str, ":{col}").unwrap();
                        }
                    }
                    bt_str.push('\n');
                    subframeno += 1;
                }
            }
            frameno += 1;
        }
        writeln!(&mut bt_str, " (..{})",
                 frames.len() - 1).unwrap();
        bt_str
    }
}
