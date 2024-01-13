use std::io::BufWriter;
use std::{path::PathBuf, fs::File};
use std::ffi::OsString;
use std::fs;

use anyhow::{Result, Context, anyhow};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Dir,
    File,
    Other
}

pub fn easy_filenames_in_dir<P>(
    path: P
) -> Result<impl Iterator<Item = Result<(OsString, FileKind)>>>
where PathBuf: From<P>
{
    let pathbuf: PathBuf = path.into();
    Ok(fs::read_dir(&pathbuf).with_context(
        || anyhow!("can't open directory for reading: {:?}",
                   pathbuf.to_string_lossy()))?
       .map(
           move |entry_result: Result<fs::DirEntry, std::io::Error>|
                                      -> Result<(OsString, FileKind)>
           {
               let entry = entry_result.with_context(
                   || anyhow!("reading directory: {:?}", pathbuf.to_string_lossy()))?;
               let ft = entry.file_type()
                   .expect("does this fail on OSes needing stat?");
               let filename = entry.file_name();
               Ok(
                   (
                       filename,
                       if ft.is_dir() {
                           FileKind::Dir
                       } else if ft.is_file() {
                           FileKind::File
                       } else {
                           FileKind::Other
                       }
                   ))
           }))
}


pub fn easy_filepaths_in_dir<P>(
    path: P
) -> Result<impl Iterator<Item = Result<(PathBuf, FileKind)>>>
where PathBuf: From<P>,
      P: Clone
{
    let pathbuf: PathBuf = path.clone().into();
    Ok(easy_filenames_in_dir(path)?
        .map(move |v| -> Result<(PathBuf, FileKind)> {
            let (item, kind) = v?;
            let mut filepath = pathbuf.clone();
            filepath.push(item);
            Ok((filepath, kind))
        }))
}


pub fn open_log_output<P>(
    path: P
) -> Result<Box<BufWriter<File>>>
where PathBuf: From<P>,
      P: Clone
{
    let mut outp = File::options();
    outp.write(true).append(true).create(true);
    let pathb = PathBuf::from(path);
    if let Some(parent) = pathb.parent() {
        let _ignore = std::fs::create_dir(parent);
    }
    Ok(Box::new(BufWriter::new(outp.open(&pathb).with_context(
        || anyhow!("opening log for output: {:?}", pathb.to_string_lossy()))?)))
}
