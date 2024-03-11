
use anyhow::Result;
use website::{scripting::{capture_strings, xrun}, stringsplit::StringSplit};


fn make(target: &str) -> Result<()> {
    xrun("make", &[target])
}

fn cargo(args: &[&str]) -> Result<()> {
    xrun("cargo", args)
}

fn gls(dirpath: &str) -> Result<StringSplit> {
    capture_strings("git", &["ls-files", "-z", "--", dirpath], "\0")
}


fn main() -> Result<()> {
    make("test_website")?;

    cargo(&["build", "--release", "--bin", "website", "--bin", "access_control"])?;

    let mut files = vec!["deploy-receive",
                         "resources/merged/elements/",
                         "accounts-schema.sql",
                         "target/release/website",
                         "target/release/access_control"];

    let gls = gls("data")?;
    files.extend(gls.items());

    println!("Sending files: {files:?}");
    xrun("netsend", &files)?;
    
    Ok(())
}
