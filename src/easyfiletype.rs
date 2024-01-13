use std::fs::FileType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EasyFileType {
    Dir,
    File,
    Symlink,
    Other  // device files, sockets, pipes, ?
}


pub trait GetEasyFileType {
    fn easyfiletype(&self) -> EasyFileType;
}

impl GetEasyFileType for FileType {
    fn easyfiletype(&self) -> EasyFileType {
        if self.is_symlink() {
            EasyFileType::Symlink
        } else if self.is_dir() {
            EasyFileType::Dir
        } else if self.is_file() {
            EasyFileType::File
        } else {
            EasyFileType::Other
        }
    }
}
