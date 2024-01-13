use std::{path::Path, fs::read_to_string};

use anyhow::{Result, anyhow, Context};


pub fn my_read_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    read_to_string(&path).with_context(
        || anyhow!("opening path for reading: {:?}", path.as_ref()))
}
