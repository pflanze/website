//! Printing statements to stderr for debugging purposes

#[macro_export]
macro_rules! pp {
    ($namestr:expr, $val:expr) => {{
        let res = $val;
        eprintln!("{}: {:?}", $namestr, res);
        res
    }}
}

#[macro_export]
macro_rules! nopp {
    ($namestr:expr, $arg:expr) => {
        $arg
    }
}


#[macro_export]
macro_rules! warn {
    ($formatstr:expr $(,$arg:expr)*) => { {
        use std::io::Write;
        let mut outp = std::io::BufWriter::new(std::io::stderr().lock());
        let _ = write!(&mut outp, "W: ");
        let _ = write!(&mut outp, $formatstr $(,$arg)*);
        let _ = writeln!(&mut outp, " at {:?} line {}", file!(), line!());
        let _ = outp.flush();
    } }
}

#[macro_export]
macro_rules! nowarn {
    ($formatstr:expr $(,$arg:expr)*) => {
    }
}

/// Requires a `pub static DO_WARN_THREAD: AtomicBool =
/// AtomicBool::new(false);` in the scope, which can be changed via
/// `...::DO_WARN_THREAD.store(true,
/// std::sync::atomic::Ordering::SeqCst);`.
#[macro_export]
macro_rules! warn_thread {
    { $fmt:expr $(,$arg:expr)* } => {
        if DO_WARN_THREAD.load(std::sync::atomic::Ordering::SeqCst) {
            use std::io::Write;
            let mut outp = std::io::BufWriter::new(std::io::stderr().lock());
            let _ = write!(&mut outp, "{:?} W: ", std::thread::current().id());
            let _ = write!(&mut outp, $fmt $(,$arg)*);
            let _ = writeln!(&mut outp, " at {:?} line {}", file!(), line!());
            let _ = outp.flush();
        }
    }
}

#[macro_export]
macro_rules! nowarn_thread {
    ($formatstr:expr $(,$arg:expr)*) => {
    }
}


#[macro_export]
macro_rules! warn_todo {
    ($formatstr:expr $(,$arg:expr)*) => {
        use std::io::Write;
        let mut outp = std::io::BufWriter::new(std::io::stderr().lock());
        let _ = write!(&mut outp, "Todo: ");
        let _ = write!(&mut outp, $formatstr, $(,$arg)*);
        let _ = writeln!(&mut outp, " at {:?} line {}", file!(), line!());
        let _ = outp.flush();
    }
}

#[macro_export]
macro_rules! nowarn_todo {
    ($formatstr:expr $(,$arg:expr)*) => {
    }
}



