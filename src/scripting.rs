use std::process::{Command, ExitStatus};

use anyhow::{Result, anyhow, Context, bail};

use crate::{try_result, stringsplit::StringSplit};


/// Run a command without capturing anything, returning its status.
pub fn run(cmd: &str, args: &[&str]) -> Result<ExitStatus> {
    Command::new(cmd)
        .args(args)
        .status()
        .with_context(|| anyhow!("run({cmd:?}, {args:?})"))
}

/// Run a command without capturing anything, treating any non-0
/// status as an error.
pub fn xrun(cmd: &str, args: &[&str]) -> Result<()> {
    let status = run(cmd, args)?;
    if status.success() {
        Ok(())
    } else {
        bail!("run({cmd:?}, {args:?}) gave {}", status.to_string())
    }
}

/// Run a command capturing its stdout, as a `StringSplit` pre-split
/// using `separator` and suppressing the last empty line (if a
/// trailing separator appears in the output).
pub fn capture_strings(cmd: &str, args: &[&str], separator: &str) -> Result<StringSplit> {
    try_result!{
        let output = Command::new(cmd)
            .args(args)
        .output()?;
        let b = String::from_utf8(output.stdout)?.into_boxed_str();
        Ok::<_, anyhow::Error>(StringSplit::split(b, separator, true))
    }.with_context(|| anyhow!("capture_strings({cmd:?}, {args:?}, {separator:?})"))
}


