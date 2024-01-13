//! File metadata that can be used as key to verify (with good chance,
//! given good faith actors) if a file has changed on disk.

use std::{time::SystemTime, fs::Metadata, os::unix::prelude::MetadataExt};

use anyhow::Result;

use crate::easyfiletype::{GetEasyFileType, EasyFileType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmpFileMeta {
    pub easyfiletype: EasyFileType,
    pub modified_time: SystemTime,
    pub created_time: SystemTime,
    pub ino: u64,
    pub len: u64, // size
}

pub trait GetCmpFileMeta {
    fn cmpfilemeta(&self) -> Result<CmpFileMeta>;
}

impl GetCmpFileMeta for Metadata {
    fn cmpfilemeta(&self) -> Result<CmpFileMeta> {
        Ok(CmpFileMeta {
            easyfiletype: self.file_type().easyfiletype(),
            modified_time: self.modified()?,
            created_time: self.created()?,
            ino: self.ino(),
            len: self.len()
        })
    }
}

