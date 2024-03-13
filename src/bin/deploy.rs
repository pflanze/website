
use anyhow::Result;
use website::{scripting::{capture_strings, xrun}, stringsplit::StringSplit};

fn gls(dirpath: &str) -> Result<StringSplit> {
    capture_strings("git", &["ls-files", "-z", "--", dirpath], "\0")
}


fn main() -> Result<()> {
    xrun("make", &["test_website"])?;

    xrun("cargo", &["build", "--release", "--bin", "website", "--bin", "access_control"])?;

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
